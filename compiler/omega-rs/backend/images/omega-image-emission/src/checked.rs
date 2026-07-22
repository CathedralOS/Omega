use crate::dispatch::emit_executable_image;
use crate::input::ExecutableImageInput;
use omega_core::diagnostics::Diagnostic;
use omega_image::{
    EmittedImageOutput, FinalExecutableRegionOrigin, PlacedExecutableRegionInventory,
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
    let relocations = input.relocations;
    if let Some(emitted_output) = emit_executable_image(input) {
        let emitted_output = emitted_output?;
        validate_final_text_relocation_envelope(
            encoded_text_bytes,
            &emitted_output.final_text_bytes,
            relocations,
        )?;
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

/// Prove that final `.text` preserves every encoded bit except the exact
/// immediate fields named by checked relocation records. A relocation may
/// change an address or displacement, never an instruction opcode/register.
fn validate_final_text_relocation_envelope(
    encoded_text_bytes: &[u8],
    final_text_bytes: &[u8],
    relocations: &RelocationPlan,
) -> Result<(), Diagnostic> {
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
    Ok(())
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
        validate_final_text_relocation_envelope,
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

        validate_final_text_relocation_envelope(&encoded, &relocated, &relocations)
            .expect("declared displacement bytes may change");
        relocated[0] = 0x90;
        let diagnostic =
            validate_final_text_relocation_envelope(&encoded, &relocated, &relocations)
                .expect_err("an opcode mutation outside the displacement must reject");
        assert!(diagnostic.message.contains("byte 0"));
    }
}
