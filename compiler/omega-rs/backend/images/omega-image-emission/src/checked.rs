use crate::dispatch::emit_executable_image;
use crate::input::ExecutableImageInput;
use omega_core::diagnostics::Diagnostic;
use omega_image::{
    CompilerTextValidationEvidence, EmittedImageOutput, FinalExecutableRegionOrigin,
    PlacedExecutableRegionInventory,
};
use omega_object_file::{RelocationKind, RelocationPlan, SectionKind};
use omega_target::Architecture;

pub fn emit_checked_executable_image(
    input: ExecutableImageInput<'_>,
    planned_text_bytes: usize,
) -> Result<EmittedImageOutput, Diagnostic> {
    if input.text_bytes.len() != planned_text_bytes {
        return Err(Diagnostic::error(format!(
            "cannot emit native output for {:?}: encoded {} machine byte(s), planned {} byte(s)",
            input.target,
            input.text_bytes.len(),
            planned_text_bytes
        )));
    }

    let architecture = input.target.architecture;
    let entry_symbol = omega_object_file::object_entry_symbol_name(input.object).to_owned();
    let encoded_text_bytes = input.text_bytes;
    if input.encoded_machine_code.bytes.storage_slice() != encoded_text_bytes {
        return Err(Diagnostic::error(
            "checked image input text does not match its encoded-machine byte carrier",
        ));
    }
    let encoded_machine_code = input.encoded_machine_code;
    let relocations = input.relocations;
    if let Some(emitted_output) = emit_executable_image(input) {
        let mut emitted_output = emitted_output?;
        let mut compiler_text_validation = validate_final_text_relocation_envelope(
            encoded_text_bytes,
            &emitted_output.final_text_bytes,
            relocations,
        )?;
        let (fixed_checked_instruction_count, fixed_checked_instruction_fingerprint) =
            validate_fixed_checked_instruction_bytes(
                architecture,
                encoded_machine_code,
                &emitted_output.final_text_bytes,
            )?;
        compiler_text_validation.fixed_checked_instruction_count = fixed_checked_instruction_count;
        compiler_text_validation.fixed_checked_instruction_fingerprint =
            fixed_checked_instruction_fingerprint;
        let mut derivation_fingerprint = 0xcbf2_9ce4_8422_2325u64;
        fingerprint_into(
            &mut derivation_fingerprint,
            &compiler_text_validation
                .derivation_fingerprint
                .to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &fixed_checked_instruction_fingerprint.to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &(fixed_checked_instruction_count as u64).to_le_bytes(),
        );
        compiler_text_validation.derivation_fingerprint = derivation_fingerprint;
        emitted_output.compiler_text_validation = Some(compiler_text_validation);
        validate_executable_region_enumeration(&emitted_output.executable_regions)?;
        validate_compiler_entry_call_return_bytes(
            architecture,
            &entry_symbol,
            &emitted_output.final_text_bytes,
            &emitted_output.executable_regions,
        )?;
        return Ok(emitted_output);
    }

    Err(Diagnostic::error(
        "cannot emit native executable; no direct image writer is registered for this target",
    ))
}

fn validate_executable_region_enumeration(
    inventory: &PlacedExecutableRegionInventory,
) -> Result<(), Diagnostic> {
    if let Some(gap) = inventory.unclassified_gaps.first() {
        return Err(Diagnostic::error(format!(
            "final executable region enumeration left {} unclassified byte(s) at .text offset {}",
            gap.byte_count, gap.section_offset
        )));
    }
    Ok(())
}

/// Prove that final `.text` preserves every encoded bit except the exact
/// immediate fields named by checked relocation records. A relocation may
/// change an address or displacement, never an instruction opcode/register.
fn validate_final_text_relocation_envelope(
    encoded_text_bytes: &[u8],
    final_text_bytes: &[u8],
    relocations: &RelocationPlan,
) -> Result<CompilerTextValidationEvidence, Diagnostic> {
    if final_text_bytes.len() < encoded_text_bytes.len() {
        return Err(Diagnostic::error(format!(
            "relocated .text truncated compiler code from {} to {} byte(s)",
            encoded_text_bytes.len(),
            final_text_bytes.len()
        )));
    }
    // Format-owned thunks may follow the compiler-authored prefix and have
    // their own exact final-byte validators in the image writers.
    let final_compiler_text = &final_text_bytes[..encoded_text_bytes.len()];
    let mut mutable_bits = vec![0u8; encoded_text_bytes.len()];
    let mut text_relocations = Vec::new();
    for (_, relocation) in relocations.records() {
        if relocation.section != SectionKind::Text {
            continue;
        }
        let (expected_width, masks): (usize, &[u8]) = match relocation.kind {
            RelocationKind::X86_64Relative32 => (4, &[0xff; 4]),
            RelocationKind::Absolute64 => (8, &[0xff; 8]),
            RelocationKind::Aarch64Page21 => {
                // ADRP immlo[30:29] and immhi[23:5].
                (4, &[0xe0, 0xff, 0xff, 0x60])
            }
            RelocationKind::Aarch64PageOffset12 => {
                // ADD/LDR unsigned immediate bits [21:10].
                (4, &[0x00, 0xfc, 0x3f, 0x00])
            }
            RelocationKind::Aarch64Branch26 => {
                // B/BL immediate bits [25:0].
                (4, &[0xff, 0xff, 0xff, 0x03])
            }
        };
        if relocation.byte_width != expected_width {
            return Err(Diagnostic::error(format!(
                "text relocation at byte {} has width {}, expected {} for {:?}",
                relocation.offset, relocation.byte_width, expected_width, relocation.kind
            )));
        }
        let end = relocation
            .offset
            .checked_add(expected_width)
            .filter(|end| *end <= mutable_bits.len())
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "text relocation at byte {} exceeds encoded .text",
                    relocation.offset
                ))
            })?;
        if mutable_bits[relocation.offset..end]
            .iter()
            .any(|mask| *mask != 0)
        {
            return Err(Diagnostic::error(format!(
                "text relocation at byte {} overlaps another relocation field",
                relocation.offset
            )));
        }
        mutable_bits[relocation.offset..end].copy_from_slice(masks);
        text_relocations.push((
            relocation.offset,
            relocation.byte_width,
            relocation_kind_tag(relocation.kind),
        ));
    }

    for (offset, ((encoded, final_byte), mutable_mask)) in encoded_text_bytes
        .iter()
        .zip(final_compiler_text)
        .zip(&mutable_bits)
        .enumerate()
    {
        let changed_bits = encoded ^ final_byte;
        if changed_bits & !mutable_mask != 0 {
            return Err(Diagnostic::error(format!(
                "final compiler .text byte {offset} changed outside its declared relocation field"
            )));
        }
    }
    text_relocations.sort_unstable();
    let encoded_text_fingerprint = fingerprint_bytes(encoded_text_bytes);
    let final_compiler_text_fingerprint = fingerprint_bytes(final_compiler_text);
    let mut relocation_envelope_fingerprint = 0xcbf2_9ce4_8422_2325u64;
    for (offset, width, kind) in &text_relocations {
        fingerprint_into(
            &mut relocation_envelope_fingerprint,
            &(*offset as u64).to_le_bytes(),
        );
        fingerprint_into(
            &mut relocation_envelope_fingerprint,
            &(*width as u64).to_le_bytes(),
        );
        fingerprint_into(&mut relocation_envelope_fingerprint, &[*kind]);
    }
    let mut derivation_fingerprint = 0xcbf2_9ce4_8422_2325u64;
    fingerprint_into(
        &mut derivation_fingerprint,
        &encoded_text_fingerprint.to_le_bytes(),
    );
    fingerprint_into(
        &mut derivation_fingerprint,
        &final_compiler_text_fingerprint.to_le_bytes(),
    );
    fingerprint_into(
        &mut derivation_fingerprint,
        &relocation_envelope_fingerprint.to_le_bytes(),
    );
    fingerprint_into(
        &mut derivation_fingerprint,
        &(text_relocations.len() as u64).to_le_bytes(),
    );
    Ok(CompilerTextValidationEvidence {
        encoded_text_fingerprint,
        final_compiler_text_fingerprint,
        relocation_envelope_fingerprint,
        fixed_checked_instruction_fingerprint: 0,
        derivation_fingerprint,
        text_relocation_count: text_relocations.len(),
        fixed_checked_instruction_count: 0,
    })
}

/// Validate the exact final encodings of the first closed checked-assembly
/// subset. Instruction boundaries come from the encoded carrier; arbitrary
/// byte scanning could mistake immediates or data for opcodes.
fn validate_fixed_checked_instruction_bytes(
    architecture: Architecture,
    code: &omega_machine_bytes::EncodedMachineCode,
    final_text_bytes: &[u8],
) -> Result<(usize, u64), Diagnostic> {
    use omega_machine_bytes::FixedCheckedInstructionKind;

    let mut count = 0usize;
    let mut fingerprint = 0xcbf2_9ce4_8422_2325u64;
    for (_, instruction) in code.instructions.iter() {
        let Some(kind) = instruction.fixed_checked_kind else {
            continue;
        };
        if architecture != Architecture::X86_64 {
            return Err(Diagnostic::error(
                "fixed checked-assembly validation found an x86 instruction on a non-x86 target",
            ));
        }
        if instruction.bytes.is_empty() || !instruction.bytes.start().is_valid() {
            return Err(Diagnostic::error(format!(
                "fixed checked-assembly instruction #{} has no encoded byte span",
                instruction.selected_instruction_index
            )));
        }
        let byte_offset = instruction.bytes.start().arena_index() as usize - 1;
        let byte_count = instruction.bytes.len();
        let byte_end = byte_offset
            .checked_add(byte_count)
            .filter(|end| *end <= final_text_bytes.len())
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "fixed checked-assembly instruction #{} exceeds final compiler text",
                    instruction.selected_instruction_index
                ))
            })?;
        let encoded_bytes = code.bytes.span(instruction.bytes).ok_or_else(|| {
            Diagnostic::error(format!(
                "fixed checked-assembly instruction #{} names an invalid encoded byte span",
                instruction.selected_instruction_index
            ))
        })?;
        let expected: &[u8] = match kind {
            FixedCheckedInstructionKind::MachineHalt => &[0xf4],
            FixedCheckedInstructionKind::LoadFence => &[0x0f, 0xae, 0xe8],
            FixedCheckedInstructionKind::StoreFence => &[0x0f, 0xae, 0xf8],
            FixedCheckedInstructionKind::FullFence => &[0x0f, 0xae, 0xf0],
            FixedCheckedInstructionKind::InterruptDisable => &[0xfa],
            FixedCheckedInstructionKind::InterruptEnable => &[0xfb],
        };
        if encoded_bytes != expected {
            return Err(Diagnostic::error(format!(
                "encoded checked-assembly instruction #{} does not match its closed catalog kind",
                instruction.selected_instruction_index
            )));
        }
        if &final_text_bytes[byte_offset..byte_end] != expected {
            return Err(Diagnostic::error(format!(
                "final checked-assembly instruction #{} changed after encoding",
                instruction.selected_instruction_index
            )));
        }

        let kind_tag = match kind {
            FixedCheckedInstructionKind::MachineHalt => 1,
            FixedCheckedInstructionKind::LoadFence => 2,
            FixedCheckedInstructionKind::StoreFence => 3,
            FixedCheckedInstructionKind::FullFence => 4,
            FixedCheckedInstructionKind::InterruptDisable => 5,
            FixedCheckedInstructionKind::InterruptEnable => 6,
        };
        fingerprint_into(&mut fingerprint, &[kind_tag]);
        fingerprint_into(
            &mut fingerprint,
            &u64::from(instruction.selected_instruction_index).to_le_bytes(),
        );
        fingerprint_into(&mut fingerprint, &(byte_offset as u64).to_le_bytes());
        fingerprint_into(&mut fingerprint, expected);
        count += 1;
    }
    Ok((count, fingerprint))
}

fn relocation_kind_tag(kind: RelocationKind) -> u8 {
    match kind {
        RelocationKind::Aarch64Page21 => 1,
        RelocationKind::Aarch64PageOffset12 => 2,
        RelocationKind::Aarch64Branch26 => 3,
        RelocationKind::Absolute64 => 4,
        RelocationKind::X86_64Relative32 => 5,
    }
}

fn fingerprint_bytes(bytes: &[u8]) -> u64 {
    let mut fingerprint = 0xcbf2_9ce4_8422_2325u64;
    fingerprint_into(&mut fingerprint, bytes);
    fingerprint
}

fn fingerprint_into(fingerprint: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *fingerprint ^= u64::from(*byte);
        *fingerprint = fingerprint.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

fn validate_compiler_entry_call_return_bytes(
    architecture: Architecture,
    entry_symbol: &str,
    final_text_bytes: &[u8],
    inventory: &PlacedExecutableRegionInventory,
) -> Result<(), Diagnostic> {
    let matching_entries = inventory
        .regions
        .iter()
        .filter(|region| {
            region.origin == FinalExecutableRegionOrigin::CompilerFunction
                && region.symbol == entry_symbol
        })
        .collect::<Vec<_>>();
    if matching_entries.len() != 1 {
        return Err(Diagnostic::error(format!(
            "final-byte validation requires exactly one compiler entry region named \
             `{entry_symbol}`; found {}",
            matching_entries.len()
        )));
    }
    let entry = matching_entries[0];
    let entry_end = entry
        .section_offset
        .checked_add(entry.byte_count)
        .filter(|end| *end <= final_text_bytes.len())
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "compiler entry region `{entry_symbol}` exceeds relocated .text during final-byte validation"
            ))
        })?;
    let bytes = &final_text_bytes[entry.section_offset..entry_end];
    let (prologue, epilogue): (Vec<u8>, Vec<u8>) = match architecture {
        Architecture::X86_64 => (
            omega_isa_x86_64::encode_function_enter_bytes().to_vec(),
            omega_isa_x86_64::encode_return_bytes().to_vec(),
        ),
        Architecture::Aarch64 => (
            omega_isa_aarch64::encode_function_enter_bytes().to_vec(),
            omega_isa_aarch64::encode_return_bytes().to_vec(),
        ),
    };
    if bytes.len() < prologue.len() + epilogue.len() {
        return Err(Diagnostic::error(format!(
            "compiler entry region `{entry_symbol}` is too short for its fixed call-return mechanics"
        )));
    }
    if !bytes.starts_with(&prologue) {
        return Err(Diagnostic::error(format!(
            "compiler entry region `{entry_symbol}` has invalid final function-entry bytes"
        )));
    }
    if !bytes.ends_with(&epilogue) {
        return Err(Diagnostic::error(format!(
            "compiler entry region `{entry_symbol}` has invalid final function-return bytes"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        emit_checked_executable_image, validate_compiler_entry_call_return_bytes,
        validate_executable_region_enumeration, validate_final_text_relocation_envelope,
        validate_fixed_checked_instruction_bytes,
    };
    use crate::ExecutableImageInput;
    use omega_core::arena::Handle;
    use omega_image::{
        FinalExecutableRegionOrigin, PlacedExecutableRegion, PlacedExecutableRegionInventory,
    };
    use omega_object_file::{
        ObjectPlan, RelocationKind, RelocationOrigin, RelocationPlan, RelocationRecord, SectionKind,
    };
    use omega_target::NativeTarget;

    #[test]
    fn rejects_native_image_when_encoded_text_size_differs_from_plan() {
        let target = NativeTarget::linux_arm64();
        let object = ObjectPlan::with_capacity(target, 0, 0);
        let relocations = RelocationPlan::with_target(target);

        let diagnostic = emit_checked_executable_image(
            ExecutableImageInput {
                target,
                object: &object,
                relocations: &relocations,
                encoded_machine_code: &omega_machine_bytes::EncodedMachinePlan::with_capacity(
                    target, 0, 0, 0,
                )
                .code,
                text_bytes: &[0xaa, 0xbb],
                data_bytes: &[],
                subsystem: 3,
            },
            4,
        )
        .expect_err("encoded/planned byte mismatch should fail before image dispatch");

        assert!(diagnostic.message.contains("encoded 2 machine byte(s)"));
        assert!(diagnostic.message.contains("planned 4 byte(s)"));
    }

    #[test]
    fn validates_fixed_call_return_mechanics_in_relocated_entry_bytes() {
        let prologue = omega_isa_x86_64::encode_function_enter_bytes();
        let epilogue = omega_isa_x86_64::encode_return_bytes();
        let mut bytes = prologue
            .into_iter()
            .chain([0x90])
            .chain(epilogue)
            .collect::<Vec<_>>();
        let inventory = PlacedExecutableRegionInventory {
            text_address: 0x1000,
            text_byte_count: bytes.len(),
            text_fingerprint: 1,
            inventory_fingerprint: 2,
            regions: vec![PlacedExecutableRegion {
                origin: FinalExecutableRegionOrigin::CompilerFunction,
                section_offset: 0,
                address: 0x1000,
                byte_count: bytes.len(),
                byte_fingerprint: 3,
                symbol: "entry".into(),
                footprint: None,
            }],
            unclassified_gaps: Vec::new(),
        };

        validate_compiler_entry_call_return_bytes(
            omega_target::Architecture::X86_64,
            "entry",
            &bytes,
            &inventory,
        )
        .expect("exact encoder-owned mechanics should validate");
        bytes[0] ^= 0xff;
        let diagnostic = validate_compiler_entry_call_return_bytes(
            omega_target::Architecture::X86_64,
            "entry",
            &bytes,
            &inventory,
        )
        .expect_err("mutated final mechanics must reject");
        assert!(diagnostic.message.contains("function-entry bytes"));
    }

    #[test]
    fn final_text_changes_only_inside_declared_relocation_bits() {
        let encoded = [0xe8, 0, 0, 0, 0, 0xc3];
        let mut relocated = encoded;
        relocated[1..5].copy_from_slice(&0x1234_5678u32.to_le_bytes());
        let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Instruction {
                function_symbol_handle: Handle::invalid(),
                selected_instruction_index: 1,
            },
            section: SectionKind::Text,
            offset: 1,
            byte_width: 4,
            symbol_handle: Handle::invalid(),
            kind: RelocationKind::X86_64Relative32,
        });

        let evidence = validate_final_text_relocation_envelope(&encoded, &relocated, &relocations)
            .expect("declared displacement bytes may change");
        assert_eq!(evidence.text_relocation_count, 1);
        assert_ne!(evidence.encoded_text_fingerprint, 0);
        assert_ne!(evidence.derivation_fingerprint, 0);
        relocated[0] = 0x90;
        let diagnostic =
            validate_final_text_relocation_envelope(&encoded, &relocated, &relocations)
                .expect_err("an opcode mutation outside the displacement must reject");
        assert!(diagnostic.message.contains("byte 0"));
    }

    #[test]
    fn checked_emission_rejects_unclassified_executable_bytes() {
        let inventory = PlacedExecutableRegionInventory {
            text_address: 0x1000,
            text_byte_count: 4,
            text_fingerprint: 1,
            inventory_fingerprint: 2,
            regions: Vec::new(),
            unclassified_gaps: vec![omega_image::PlacedExecutableGap {
                section_offset: 0,
                address: 0x1000,
                byte_count: 4,
                byte_fingerprint: 3,
            }],
        };

        let diagnostic = validate_executable_region_enumeration(&inventory)
            .expect_err("checked images must classify every executable byte");
        assert!(diagnostic.message.contains("4 unclassified byte(s)"));
    }

    #[test]
    fn validates_fixed_checked_assembly_at_retained_instruction_boundaries() {
        use omega_core::arena::Arena;
        use omega_machine_bytes::{
            EncodedMachineCode, EncodedMachineInstruction, FixedCheckedInstructionKind,
        };

        let mut bytes = Arena::with_capacity(5);
        let halt = bytes.insert_many([0xf4]);
        let fence = bytes.insert_many([0x0f, 0xae, 0xf0]);
        let cli = bytes.insert_many([0xfa]);
        let mut instructions = Arena::with_capacity(3);
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 4,
            bytes: halt,
            fixed_checked_kind: Some(FixedCheckedInstructionKind::MachineHalt),
        });
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 5,
            bytes: fence,
            fixed_checked_kind: Some(FixedCheckedInstructionKind::FullFence),
        });
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 6,
            bytes: cli,
            fixed_checked_kind: Some(FixedCheckedInstructionKind::InterruptDisable),
        });
        let code = EncodedMachineCode {
            functions: Arena::new(),
            instructions,
            bytes,
            byte_count: 5,
        };

        let (count, fingerprint) = validate_fixed_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &[0xf4, 0x0f, 0xae, 0xf0, 0xfa],
        )
        .expect("closed checked-assembly bytes should validate");
        assert_eq!(count, 3);
        assert_ne!(fingerprint, 0);

        let diagnostic = validate_fixed_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &[0xf4, 0x0f, 0xae, 0xe8, 0xfa],
        )
        .expect_err("a changed final fence kind must reject");
        assert!(diagnostic.message.contains("changed after encoding"));
    }
}
