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
        let (checked_instruction_validation_count, checked_instruction_validation_fingerprint) =
            validate_checked_instruction_bytes(
                architecture,
                encoded_machine_code,
                &emitted_output.final_text_bytes,
                relocations,
            )?;
        compiler_text_validation.checked_instruction_validation_count =
            checked_instruction_validation_count;
        compiler_text_validation.checked_instruction_validation_fingerprint =
            checked_instruction_validation_fingerprint;
        let mut derivation_fingerprint = 0xcbf2_9ce4_8422_2325u64;
        fingerprint_into(
            &mut derivation_fingerprint,
            &compiler_text_validation
                .derivation_fingerprint
                .to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &checked_instruction_validation_fingerprint.to_le_bytes(),
        );
        fingerprint_into(
            &mut derivation_fingerprint,
            &(checked_instruction_validation_count as u64).to_le_bytes(),
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
        checked_instruction_validation_fingerprint: 0,
        derivation_fingerprint,
        text_relocation_count: text_relocations.len(),
        checked_instruction_validation_count: 0,
    })
}

/// Validate the privilege-bearing final encodings of the closed checked-
/// assembly subset. Instruction boundaries and normalized operand facts come
/// from the encoded carrier; arbitrary byte scanning could mistake immediates
/// or data for opcodes.
fn validate_checked_instruction_bytes(
    architecture: Architecture,
    code: &omega_machine_bytes::EncodedMachineCode,
    final_text_bytes: &[u8],
    relocations: &RelocationPlan,
) -> Result<(usize, u64), Diagnostic> {
    use omega_machine_bytes::CheckedInstructionValidationKind;

    let mut count = 0usize;
    let mut fingerprint = 0xcbf2_9ce4_8422_2325u64;
    for (_, instruction) in code.instructions.iter() {
        let Some(kind) = instruction.checked_validation_kind else {
            continue;
        };
        if architecture != Architecture::X86_64 {
            return Err(Diagnostic::error(
                "checked-assembly validation found an x86 instruction on a non-x86 target",
            ));
        }
        if instruction.bytes.is_empty() || !instruction.bytes.start().is_valid() {
            return Err(Diagnostic::error(format!(
                "checked-assembly instruction #{} has no encoded byte span",
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
                    "checked-assembly instruction #{} exceeds final compiler text",
                    instruction.selected_instruction_index
                ))
            })?;
        let encoded_bytes = code.bytes.span(instruction.bytes).ok_or_else(|| {
            Diagnostic::error(format!(
                "checked-assembly instruction #{} names an invalid encoded byte span",
                instruction.selected_instruction_index
            ))
        })?;
        let final_bytes = &final_text_bytes[byte_offset..byte_end];
        validate_checked_instruction_kind(
            kind,
            instruction.selected_instruction_index,
            byte_offset,
            encoded_bytes,
            final_bytes,
            relocations,
        )?;
        for loader in instruction.checked_operand_loaders.into_iter().flatten() {
            validate_checked_operand_loader(
                loader,
                instruction.selected_instruction_index,
                byte_offset,
                encoded_bytes,
                final_bytes,
                relocations,
            )?;
            fingerprint_checked_operand_loader(&mut fingerprint, loader);
        }

        let kind_tag = match kind {
            CheckedInstructionValidationKind::MachineHalt => 1,
            CheckedInstructionValidationKind::LoadFence => 2,
            CheckedInstructionValidationKind::StoreFence => 3,
            CheckedInstructionValidationKind::FullFence => 4,
            CheckedInstructionValidationKind::InterruptDisable => 5,
            CheckedInstructionValidationKind::InterruptEnable => 6,
            CheckedInstructionValidationKind::PortWriteImmediatePort { .. } => 7,
            CheckedInstructionValidationKind::PortReadImmediatePort { .. } => 8,
            CheckedInstructionValidationKind::MsrReadImmediateIndex { .. } => 9,
            CheckedInstructionValidationKind::MsrWriteImmediateIndex { .. } => 10,
            CheckedInstructionValidationKind::ControlRegisterRead { .. } => 11,
            CheckedInstructionValidationKind::ControlRegisterWrite { .. } => 12,
            CheckedInstructionValidationKind::FlagsSnapshot { .. } => 13,
            CheckedInstructionValidationKind::FlagsRestore { .. } => 14,
            CheckedInstructionValidationKind::PortWriteRuntimePort { .. } => 15,
            CheckedInstructionValidationKind::PortReadRuntimePort { .. } => 16,
            CheckedInstructionValidationKind::MsrReadRuntimeIndex { .. } => 17,
            CheckedInstructionValidationKind::MsrWriteRuntimeIndex { .. } => 18,
        };
        fingerprint_into(&mut fingerprint, &[kind_tag]);
        fingerprint_into(
            &mut fingerprint,
            &u64::from(instruction.selected_instruction_index).to_le_bytes(),
        );
        fingerprint_into(&mut fingerprint, &(byte_offset as u64).to_le_bytes());
        fingerprint_into(&mut fingerprint, final_bytes);
        count += 1;
    }
    Ok((count, fingerprint))
}

fn fingerprint_checked_operand_loader(
    fingerprint: &mut u64,
    loader: omega_machine_bytes::CheckedOperandLoaderValidation,
) {
    use omega_machine_bytes::{
        CheckedOperandLoaderKind as Kind, CheckedOperandLoaderRegister as Register,
    };

    fingerprint_into(
        fingerprint,
        &[match loader.register {
            Register::R10 => 1,
            Register::R11 => 2,
        }],
    );
    fingerprint_into(fingerprint, &loader.byte_offset.to_le_bytes());
    fingerprint_into(fingerprint, &loader.byte_width.to_le_bytes());
    match loader.kind {
        Kind::Immediate { value } => {
            fingerprint_into(fingerprint, &[1]);
            fingerprint_into(fingerprint, &value.to_le_bytes());
        }
        Kind::Storage {
            byte_offset,
            byte_size,
        } => {
            fingerprint_into(fingerprint, &[2, byte_size]);
            fingerprint_into(fingerprint, &byte_offset.to_le_bytes());
        }
        Kind::Pointee {
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
        } => {
            fingerprint_into(fingerprint, &[3, byte_size]);
            fingerprint_into(fingerprint, &pointer_byte_offset.to_le_bytes());
            fingerprint_into(fingerprint, &field_byte_offset.to_le_bytes());
        }
        Kind::FrameFixedIndexed {
            descriptor_byte_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => {
            fingerprint_into(fingerprint, &[4, byte_size]);
            fingerprint_into(fingerprint, &descriptor_byte_offset.to_le_bytes());
            fingerprint_into(fingerprint, &element_index.to_le_bytes());
            fingerprint_into(fingerprint, &element_byte_size.to_le_bytes());
            fingerprint_into(fingerprint, &field_byte_offset.to_le_bytes());
        }
        Kind::FrameBaseIndexed {
            base_byte_offset,
            index_byte_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => {
            fingerprint_into(fingerprint, &[5, index_byte_size, byte_size]);
            fingerprint_into(fingerprint, &base_byte_offset.to_le_bytes());
            fingerprint_into(fingerprint, &index_byte_offset.to_le_bytes());
            fingerprint_into(fingerprint, &element_byte_size.to_le_bytes());
            fingerprint_into(fingerprint, &field_byte_offset.to_le_bytes());
        }
        Kind::FrameIndexed {
            descriptor_byte_offset,
            index_from_machine,
            index_byte_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => {
            fingerprint_into(
                fingerprint,
                &[6, u8::from(index_from_machine), index_byte_size, byte_size],
            );
            fingerprint_into(fingerprint, &descriptor_byte_offset.to_le_bytes());
            fingerprint_into(fingerprint, &index_byte_offset.to_le_bytes());
            fingerprint_into(fingerprint, &element_byte_size.to_le_bytes());
            fingerprint_into(fingerprint, &field_byte_offset.to_le_bytes());
        }
    }
}

fn validate_checked_operand_loader(
    loader: omega_machine_bytes::CheckedOperandLoaderValidation,
    selected_instruction_index: u32,
    instruction_byte_offset: usize,
    encoded_instruction: &[u8],
    final_instruction: &[u8],
    relocations: &RelocationPlan,
) -> Result<(), Diagnostic> {
    use omega_machine_bytes::{
        CheckedOperandLoaderKind as Kind, CheckedOperandLoaderRegister as Register,
    };

    let start = usize::try_from(loader.byte_offset).expect("u32 loader offset fits usize");
    let width = usize::try_from(loader.byte_width).expect("u32 loader width fits usize");
    let end = start
        .checked_add(width)
        .filter(|end| *end <= encoded_instruction.len() && *end <= final_instruction.len())
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "checked-assembly instruction #{selected_instruction_index} operand loader exceeds its retained byte span"
            ))
        })?;
    let encoded = &encoded_instruction[start..end];
    let final_bytes = &final_instruction[start..end];

    match loader.kind {
        Kind::Immediate { value } => {
            let mut expected = Vec::with_capacity(10);
            expected.extend(match loader.register {
                Register::R10 => [0x49, 0xba],
                Register::R11 => [0x49, 0xbb],
            });
            expected.extend(value.to_le_bytes());
            if width != expected.len() || encoded != expected || final_bytes != expected {
                return Err(Diagnostic::error(format!(
                    "checked-assembly instruction #{selected_instruction_index} immediate operand loader does not match its retained value/register semantics"
                )));
            }
        }
        Kind::Storage {
            byte_offset,
            byte_size,
        } => {
            let displacement = i32::try_from(byte_offset).map_err(|_| {
                Diagnostic::error(format!(
                    "checked-assembly instruction #{selected_instruction_index} storage operand displacement does not fit x86 disp32"
                ))
            })?;
            let opcode: &[u8] = match (loader.register, byte_size) {
                (Register::R10, 1) => &[0x45, 0x8a, 0x97],
                (Register::R10, 2) => &[0x66, 0x45, 0x8b, 0x97],
                (Register::R10, 4) => &[0x45, 0x8b, 0x97],
                (Register::R10, 8) => &[0x4d, 0x8b, 0x97],
                (Register::R11, 1) => &[0x45, 0x8a, 0x9f],
                (Register::R11, 2) => &[0x66, 0x45, 0x8b, 0x9f],
                (Register::R11, 4) => &[0x45, 0x8b, 0x9f],
                (Register::R11, 8) => &[0x4d, 0x8b, 0x9f],
                _ => {
                    return Err(Diagnostic::error(format!(
                        "checked-assembly instruction #{selected_instruction_index} retains unsupported {byte_size}-byte storage operand semantics"
                    )));
                }
            };
            let mut suffix = Vec::with_capacity(opcode.len() + 4);
            suffix.extend(opcode);
            suffix.extend(displacement.to_le_bytes());
            let expected_width = 10 + suffix.len();
            if width != expected_width
                || encoded.get(..2) != Some(&[0x49, 0xbf])
                || encoded.get(2..10) != Some(&[0; 8])
                || encoded.get(10..) != Some(suffix.as_slice())
            {
                return Err(Diagnostic::error(format!(
                    "encoded checked-assembly instruction #{selected_instruction_index} storage operand loader does not match its retained offset/width/register semantics"
                )));
            }
            if final_bytes.get(..2) != Some(&[0x49, 0xbf])
                || final_bytes.get(10..) != Some(suffix.as_slice())
            {
                return Err(Diagnostic::error(format!(
                    "final checked-assembly instruction #{selected_instruction_index} changed its storage operand loader semantics"
                )));
            }
            require_checked_operand_storage_relocation(
                relocations,
                instruction_byte_offset + start + 2,
                selected_instruction_index,
            )?;
        }
        Kind::Pointee {
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
        } => {
            validate_checked_indirect_operand_loader(
                loader.register,
                pointer_byte_offset,
                field_byte_offset,
                byte_size,
                width,
                encoded,
                final_bytes,
                selected_instruction_index,
            )?;
            require_checked_operand_storage_relocation(
                relocations,
                instruction_byte_offset + start + 2,
                selected_instruction_index,
            )?;
        }
        Kind::FrameFixedIndexed {
            descriptor_byte_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => {
            let displacement = element_index
                .checked_mul(u64::from(element_byte_size))
                .and_then(|scaled| scaled.checked_add(u64::from(field_byte_offset)))
                .and_then(|displacement| u32::try_from(displacement).ok())
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "checked-assembly instruction #{selected_instruction_index} fixed-index operand displacement overflows its retained range"
                    ))
                })?;
            validate_checked_indirect_operand_loader(
                loader.register,
                descriptor_byte_offset,
                displacement,
                byte_size,
                width,
                encoded,
                final_bytes,
                selected_instruction_index,
            )?;
            require_checked_operand_storage_relocation(
                relocations,
                instruction_byte_offset + start + 2,
                selected_instruction_index,
            )?;
        }
        Kind::FrameBaseIndexed {
            base_byte_offset,
            index_byte_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => {
            validate_checked_frame_base_indexed_operand_loader(
                loader.register,
                base_byte_offset,
                index_byte_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                byte_size,
                width,
                encoded,
                final_bytes,
                selected_instruction_index,
            )?;
            require_checked_operand_storage_relocation(
                relocations,
                instruction_byte_offset + start + 2,
                selected_instruction_index,
            )?;
        }
        Kind::FrameIndexed {
            descriptor_byte_offset,
            index_from_machine,
            index_byte_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => {
            validate_checked_frame_indexed_operand_loader(
                loader.register,
                descriptor_byte_offset,
                index_from_machine,
                index_byte_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                byte_size,
                width,
                encoded,
                final_bytes,
                selected_instruction_index,
            )?;
            require_checked_operand_storage_relocation(
                relocations,
                instruction_byte_offset + start + 2,
                selected_instruction_index,
            )?;
            if index_from_machine {
                require_checked_operand_storage_relocation(
                    relocations,
                    instruction_byte_offset + start + 17 + 2,
                    selected_instruction_index,
                )?;
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_checked_frame_indexed_operand_loader(
    register: omega_machine_bytes::CheckedOperandLoaderRegister,
    descriptor_byte_offset: u32,
    index_from_machine: bool,
    index_byte_offset: u32,
    index_byte_size: u8,
    element_byte_size: u32,
    field_byte_offset: u32,
    byte_size: u8,
    width: usize,
    encoded: &[u8],
    final_bytes: &[u8],
    selected_instruction_index: u32,
) -> Result<(), Diagnostic> {
    use omega_machine_bytes::CheckedOperandLoaderRegister as Register;

    let descriptor_displacement = i32::try_from(descriptor_byte_offset).map_err(|_| {
        Diagnostic::error(format!(
            "checked-assembly instruction #{selected_instruction_index} frame-indexed descriptor displacement does not fit x86 disp32"
        ))
    })?;
    let index_displacement = i32::try_from(index_byte_offset).map_err(|_| {
        Diagnostic::error(format!(
            "checked-assembly instruction #{selected_instruction_index} frame-indexed index displacement does not fit x86 disp32"
        ))
    })?;
    let element_scale = i32::try_from(element_byte_size).map_err(|_| {
        Diagnostic::error(format!(
            "checked-assembly instruction #{selected_instruction_index} frame-indexed element scale does not fit x86 imm32"
        ))
    })?;
    let value_displacement = i32::try_from(field_byte_offset).map_err(|_| {
        Diagnostic::error(format!(
            "checked-assembly instruction #{selected_instruction_index} frame-indexed value displacement does not fit x86 disp32"
        ))
    })?;
    let index_opcode: &[u8] = match index_byte_size {
        1 => &[0x45, 0x0f, 0xb6, 0x9f],
        2 => &[0x45, 0x0f, 0xb7, 0x9f],
        4 => &[0x45, 0x8b, 0x9f],
        8 => &[0x4d, 0x8b, 0x9f],
        _ => {
            return Err(Diagnostic::error(format!(
                "checked-assembly instruction #{selected_instruction_index} retains unsupported {index_byte_size}-byte frame-index semantics"
            )));
        }
    };
    let value_opcode: &[u8] = match (register, byte_size) {
        (Register::R10, 1) => &[0x44, 0x8a, 0x90],
        (Register::R10, 2) => &[0x66, 0x44, 0x8b, 0x90],
        (Register::R10, 4) => &[0x44, 0x8b, 0x90],
        (Register::R10, 8) => &[0x4c, 0x8b, 0x90],
        (Register::R11, 1) => &[0x44, 0x8a, 0x98],
        (Register::R11, 2) => &[0x66, 0x44, 0x8b, 0x98],
        (Register::R11, 4) => &[0x44, 0x8b, 0x98],
        (Register::R11, 8) => &[0x4c, 0x8b, 0x98],
        _ => {
            return Err(Diagnostic::error(format!(
                "checked-assembly instruction #{selected_instruction_index} retains unsupported {byte_size}-byte frame-indexed value semantics"
            )));
        }
    };
    let mut expected = Vec::with_capacity(width);
    expected.extend([0x49, 0xbf]);
    expected.extend(0u64.to_le_bytes());
    expected.extend([0x49, 0x8b, 0x87]);
    expected.extend(descriptor_displacement.to_le_bytes());
    if index_from_machine {
        expected.extend([0x49, 0xbf]);
        expected.extend(0u64.to_le_bytes());
    }
    expected.extend(index_opcode);
    expected.extend(index_displacement.to_le_bytes());
    expected.extend([0x4d, 0x69, 0xdb]);
    expected.extend(element_scale.to_le_bytes());
    expected.extend([0x4c, 0x01, 0xd8]);
    expected.extend(value_opcode);
    expected.extend(value_displacement.to_le_bytes());
    if width != expected.len() || encoded != expected {
        return Err(Diagnostic::error(format!(
            "encoded checked-assembly instruction #{selected_instruction_index} frame-indexed operand loader does not match its retained descriptor/index/scale/value semantics"
        )));
    }
    let mut expected_final = expected;
    expected_final[2..10].copy_from_slice(&final_bytes[2..10]);
    if index_from_machine {
        expected_final[19..27].copy_from_slice(&final_bytes[19..27]);
    }
    if final_bytes != expected_final {
        return Err(Diagnostic::error(format!(
            "final checked-assembly instruction #{selected_instruction_index} changed its frame-indexed operand loader semantics"
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_checked_frame_base_indexed_operand_loader(
    register: omega_machine_bytes::CheckedOperandLoaderRegister,
    base_byte_offset: u32,
    index_byte_offset: u32,
    index_byte_size: u8,
    element_byte_size: u32,
    field_byte_offset: u32,
    byte_size: u8,
    width: usize,
    encoded: &[u8],
    final_bytes: &[u8],
    selected_instruction_index: u32,
) -> Result<(), Diagnostic> {
    use omega_machine_bytes::CheckedOperandLoaderRegister as Register;

    let index_displacement = i32::try_from(index_byte_offset).map_err(|_| {
        Diagnostic::error(format!(
            "checked-assembly instruction #{selected_instruction_index} indexed operand index displacement does not fit x86 disp32"
        ))
    })?;
    let element_scale = i32::try_from(element_byte_size).map_err(|_| {
        Diagnostic::error(format!(
            "checked-assembly instruction #{selected_instruction_index} indexed operand element scale does not fit x86 imm32"
        ))
    })?;
    let value_byte_offset = base_byte_offset
        .checked_add(field_byte_offset)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "checked-assembly instruction #{selected_instruction_index} indexed operand value displacement overflows its retained range"
            ))
        })?;
    let value_displacement = i32::try_from(value_byte_offset).map_err(|_| {
        Diagnostic::error(format!(
            "checked-assembly instruction #{selected_instruction_index} indexed operand value displacement does not fit x86 disp32"
        ))
    })?;
    let index_opcode: &[u8] = match index_byte_size {
        1 => &[0x45, 0x0f, 0xb6, 0x9f],
        2 => &[0x45, 0x0f, 0xb7, 0x9f],
        4 => &[0x45, 0x8b, 0x9f],
        8 => &[0x4d, 0x8b, 0x9f],
        _ => {
            return Err(Diagnostic::error(format!(
                "checked-assembly instruction #{selected_instruction_index} retains unsupported {index_byte_size}-byte index semantics"
            )));
        }
    };
    let value_opcode: &[u8] = match (register, byte_size) {
        (Register::R10, 1) => &[0x44, 0x8a, 0x90],
        (Register::R10, 2) => &[0x66, 0x44, 0x8b, 0x90],
        (Register::R10, 4) => &[0x44, 0x8b, 0x90],
        (Register::R10, 8) => &[0x4c, 0x8b, 0x90],
        (Register::R11, 1) => &[0x44, 0x8a, 0x98],
        (Register::R11, 2) => &[0x66, 0x44, 0x8b, 0x98],
        (Register::R11, 4) => &[0x44, 0x8b, 0x98],
        (Register::R11, 8) => &[0x4c, 0x8b, 0x98],
        _ => {
            return Err(Diagnostic::error(format!(
                "checked-assembly instruction #{selected_instruction_index} retains unsupported {byte_size}-byte indexed value semantics"
            )));
        }
    };
    let mut suffix = Vec::with_capacity(index_opcode.len() + value_opcode.len() + 21);
    suffix.extend(index_opcode);
    suffix.extend(index_displacement.to_le_bytes());
    suffix.extend([0x4d, 0x69, 0xdb]);
    suffix.extend(element_scale.to_le_bytes());
    suffix.extend([0x4c, 0x89, 0xf8]);
    suffix.extend([0x4c, 0x01, 0xd8]);
    suffix.extend(value_opcode);
    suffix.extend(value_displacement.to_le_bytes());
    let expected_width = 10 + suffix.len();
    if width != expected_width
        || encoded.get(..2) != Some(&[0x49, 0xbf])
        || encoded.get(2..10) != Some(&[0; 8])
        || encoded.get(10..) != Some(suffix.as_slice())
    {
        return Err(Diagnostic::error(format!(
            "encoded checked-assembly instruction #{selected_instruction_index} frame-base-indexed operand loader does not match its retained base/index/scale/value semantics"
        )));
    }
    if final_bytes.get(..2) != Some(&[0x49, 0xbf])
        || final_bytes.get(10..) != Some(suffix.as_slice())
    {
        return Err(Diagnostic::error(format!(
            "final checked-assembly instruction #{selected_instruction_index} changed its frame-base-indexed operand loader semantics"
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_checked_indirect_operand_loader(
    register: omega_machine_bytes::CheckedOperandLoaderRegister,
    pointer_byte_offset: u32,
    value_byte_offset: u32,
    byte_size: u8,
    width: usize,
    encoded: &[u8],
    final_bytes: &[u8],
    selected_instruction_index: u32,
) -> Result<(), Diagnostic> {
    use omega_machine_bytes::CheckedOperandLoaderRegister as Register;

    let pointer_displacement = i32::try_from(pointer_byte_offset).map_err(|_| {
        Diagnostic::error(format!(
            "checked-assembly instruction #{selected_instruction_index} indirect pointer displacement does not fit x86 disp32"
        ))
    })?;
    let value_displacement = i32::try_from(value_byte_offset).map_err(|_| {
        Diagnostic::error(format!(
            "checked-assembly instruction #{selected_instruction_index} indirect value displacement does not fit x86 disp32"
        ))
    })?;
    let value_opcode: &[u8] = match (register, byte_size) {
        (Register::R10, 1) => &[0x44, 0x8a, 0x90],
        (Register::R10, 2) => &[0x66, 0x44, 0x8b, 0x90],
        (Register::R10, 4) => &[0x44, 0x8b, 0x90],
        (Register::R10, 8) => &[0x4c, 0x8b, 0x90],
        (Register::R11, 1) => &[0x44, 0x8a, 0x98],
        (Register::R11, 2) => &[0x66, 0x44, 0x8b, 0x98],
        (Register::R11, 4) => &[0x44, 0x8b, 0x98],
        (Register::R11, 8) => &[0x4c, 0x8b, 0x98],
        _ => {
            return Err(Diagnostic::error(format!(
                "checked-assembly instruction #{selected_instruction_index} retains unsupported {byte_size}-byte indirect operand semantics"
            )));
        }
    };
    let mut suffix = Vec::with_capacity(7 + value_opcode.len() + 4);
    suffix.extend([0x49, 0x8b, 0x87]);
    suffix.extend(pointer_displacement.to_le_bytes());
    suffix.extend(value_opcode);
    suffix.extend(value_displacement.to_le_bytes());
    let expected_width = 10 + suffix.len();
    if width != expected_width
        || encoded.get(..2) != Some(&[0x49, 0xbf])
        || encoded.get(2..10) != Some(&[0; 8])
        || encoded.get(10..) != Some(suffix.as_slice())
    {
        return Err(Diagnostic::error(format!(
            "encoded checked-assembly instruction #{selected_instruction_index} indirect operand loader does not match its retained pointer/value/register semantics"
        )));
    }
    if final_bytes.get(..2) != Some(&[0x49, 0xbf])
        || final_bytes.get(10..) != Some(suffix.as_slice())
    {
        return Err(Diagnostic::error(format!(
            "final checked-assembly instruction #{selected_instruction_index} changed its indirect operand loader semantics"
        )));
    }
    Ok(())
}

fn require_checked_operand_storage_relocation(
    relocations: &RelocationPlan,
    expected_offset: usize,
    selected_instruction_index: u32,
) -> Result<(), Diagnostic> {
    let matching_relocations = relocations
        .records()
        .filter(|(_, relocation)| {
            relocation.section == SectionKind::Text
                && relocation.kind == RelocationKind::Absolute64
                && relocation.offset == expected_offset
                && relocation.byte_width == 8
                && relocation.origin.selected_instruction_index()
                    == Some(selected_instruction_index)
        })
        .count();
    if matching_relocations != 1 {
        return Err(Diagnostic::error(format!(
            "checked-assembly instruction #{selected_instruction_index} requires exactly one source-storage relocation at final text byte {expected_offset}; found {matching_relocations}"
        )));
    }
    Ok(())
}

fn validate_checked_instruction_kind(
    kind: omega_machine_bytes::CheckedInstructionValidationKind,
    selected_instruction_index: u32,
    byte_offset: usize,
    encoded_bytes: &[u8],
    final_bytes: &[u8],
    relocations: &RelocationPlan,
) -> Result<(), Diagnostic> {
    use omega_machine_bytes::CheckedInstructionValidationKind;

    let fixed_expected: Option<&[u8]> = match kind {
        CheckedInstructionValidationKind::MachineHalt => Some(&[0xf4]),
        CheckedInstructionValidationKind::LoadFence => Some(&[0x0f, 0xae, 0xe8]),
        CheckedInstructionValidationKind::StoreFence => Some(&[0x0f, 0xae, 0xf8]),
        CheckedInstructionValidationKind::FullFence => Some(&[0x0f, 0xae, 0xf0]),
        CheckedInstructionValidationKind::InterruptDisable => Some(&[0xfa]),
        CheckedInstructionValidationKind::InterruptEnable => Some(&[0xfb]),
        CheckedInstructionValidationKind::PortWriteImmediatePort { .. }
        | CheckedInstructionValidationKind::PortReadImmediatePort { .. }
        | CheckedInstructionValidationKind::PortWriteRuntimePort { .. }
        | CheckedInstructionValidationKind::PortReadRuntimePort { .. }
        | CheckedInstructionValidationKind::MsrReadImmediateIndex { .. }
        | CheckedInstructionValidationKind::MsrWriteImmediateIndex { .. }
        | CheckedInstructionValidationKind::MsrReadRuntimeIndex { .. }
        | CheckedInstructionValidationKind::MsrWriteRuntimeIndex { .. }
        | CheckedInstructionValidationKind::ControlRegisterRead { .. }
        | CheckedInstructionValidationKind::ControlRegisterWrite { .. }
        | CheckedInstructionValidationKind::FlagsSnapshot { .. }
        | CheckedInstructionValidationKind::FlagsRestore { .. } => None,
    };
    if let Some(expected) = fixed_expected {
        if encoded_bytes != expected {
            return Err(Diagnostic::error(format!(
                "encoded checked-assembly instruction #{selected_instruction_index} does not match its closed catalog kind"
            )));
        }
        if final_bytes != expected {
            return Err(Diagnostic::error(format!(
                "final checked-assembly instruction #{selected_instruction_index} changed after encoding"
            )));
        }
        return Ok(());
    }

    match kind {
        CheckedInstructionValidationKind::PortWriteImmediatePort {
            port,
            value_operand_byte_width,
        } => {
            let mut prefix = Vec::with_capacity(13);
            prefix.extend([0x49, 0xba]);
            prefix.extend(u64::from(port).to_le_bytes());
            prefix.extend([0x44, 0x89, 0xd2]);
            let suffix = [0x44, 0x89, 0xd8, 0xee];
            let value_end = prefix
                .len()
                .checked_add(
                    usize::try_from(value_operand_byte_width)
                        .expect("u32 operand width fits usize"),
                )
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "checked `out` instruction #{selected_instruction_index} value width overflows"
                    ))
                })?;
            let expected_len = value_end.checked_add(suffix.len()).ok_or_else(|| {
                Diagnostic::error(format!(
                    "checked `out` instruction #{selected_instruction_index} width overflows"
                ))
            })?;
            if encoded_bytes.len() != expected_len
                || !encoded_bytes.starts_with(&prefix)
                || encoded_bytes.get(value_end..expected_len) != Some(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "encoded checked `out` instruction #{selected_instruction_index} does not bind port {port:#06x} through the closed DX/AL envelope"
                )));
            }
            if final_bytes.len() != expected_len
                || !final_bytes.starts_with(&prefix)
                || final_bytes.get(value_end..expected_len) != Some(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "final checked `out` instruction #{selected_instruction_index} changed its port or privileged opcode envelope"
                )));
            }
        }
        CheckedInstructionValidationKind::PortReadImmediatePort {
            port,
            destination_byte_offset,
        } => {
            let mut prefix = Vec::with_capacity(16);
            prefix.extend([0x49, 0xba]);
            prefix.extend(u64::from(port).to_le_bytes());
            prefix.extend([0x44, 0x89, 0xd2, 0xec, 0x49, 0xbf]);
            let mut suffix = Vec::with_capacity(7);
            suffix.extend([0x41, 0x88, 0x87]);
            suffix.extend(destination_byte_offset.to_le_bytes());
            if encoded_bytes.len() != 31
                || !encoded_bytes.starts_with(&prefix)
                || encoded_bytes[16..24] != [0; 8]
                || !encoded_bytes.ends_with(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "encoded checked `in` instruction #{selected_instruction_index} does not bind port {port:#06x} and its destination through the closed AL-store envelope"
                )));
            }
            if final_bytes.len() != 31
                || !final_bytes.starts_with(&prefix)
                || !final_bytes.ends_with(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "final checked `in` instruction #{selected_instruction_index} changed its port, privileged opcode, or destination envelope"
                )));
            }
            let destination_relocation_offset = byte_offset + 16;
            require_absolute64_text_relocation(
                relocations,
                destination_relocation_offset,
                selected_instruction_index,
                "in",
            )?;
        }
        CheckedInstructionValidationKind::PortWriteRuntimePort {
            port_operand_byte_width,
            value_operand_byte_width,
        } => {
            let port_end =
                usize::try_from(port_operand_byte_width).expect("u32 operand width fits usize");
            let value_end = port_end
                .checked_add(3)
                .and_then(|start| {
                    start.checked_add(
                        usize::try_from(value_operand_byte_width)
                            .expect("u32 operand width fits usize"),
                    )
                })
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "checked `out` instruction #{selected_instruction_index} operand widths overflow"
                    ))
                })?;
            let expected_len = value_end.checked_add(4).ok_or_else(|| {
                Diagnostic::error(format!(
                    "checked `out` instruction #{selected_instruction_index} width overflows"
                ))
            })?;
            if encoded_bytes.len() != expected_len
                || encoded_bytes.get(port_end..port_end + 3) != Some(&[0x44, 0x89, 0xd2])
                || encoded_bytes.get(value_end..expected_len) != Some(&[0x44, 0x89, 0xd8, 0xee])
            {
                return Err(Diagnostic::error(format!(
                    "encoded checked `out` instruction #{selected_instruction_index} does not preserve its runtime port/value boundaries and closed DX/AL envelope"
                )));
            }
            if final_bytes.len() != expected_len
                || final_bytes.get(port_end..port_end + 3) != Some(&[0x44, 0x89, 0xd2])
                || final_bytes.get(value_end..expected_len) != Some(&[0x44, 0x89, 0xd8, 0xee])
            {
                return Err(Diagnostic::error(format!(
                    "final checked `out` instruction #{selected_instruction_index} changed its runtime operand boundaries or privileged opcode envelope"
                )));
            }
        }
        CheckedInstructionValidationKind::PortReadRuntimePort {
            port_operand_byte_width,
            destination_byte_offset,
        } => {
            let port_end =
                usize::try_from(port_operand_byte_width).expect("u32 operand width fits usize");
            let relocation_offset = port_end.checked_add(6).ok_or_else(|| {
                Diagnostic::error(format!(
                    "checked `in` instruction #{selected_instruction_index} width overflows"
                ))
            })?;
            let expected_len = port_end.checked_add(21).ok_or_else(|| {
                Diagnostic::error(format!(
                    "checked `in` instruction #{selected_instruction_index} width overflows"
                ))
            })?;
            let mut suffix = Vec::with_capacity(7);
            suffix.extend([0x41, 0x88, 0x87]);
            suffix.extend(destination_byte_offset.to_le_bytes());
            if encoded_bytes.len() != expected_len
                || encoded_bytes.get(port_end..port_end + 6)
                    != Some(&[0x44, 0x89, 0xd2, 0xec, 0x49, 0xbf])
                || encoded_bytes.get(relocation_offset..relocation_offset + 8) != Some(&[0; 8])
                || !encoded_bytes.ends_with(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "encoded checked `in` instruction #{selected_instruction_index} does not preserve its runtime port boundary and closed AL-store envelope"
                )));
            }
            if final_bytes.len() != expected_len
                || final_bytes.get(port_end..port_end + 6)
                    != Some(&[0x44, 0x89, 0xd2, 0xec, 0x49, 0xbf])
                || !final_bytes.ends_with(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "final checked `in` instruction #{selected_instruction_index} changed its runtime port boundary, privileged opcode, or destination envelope"
                )));
            }
            require_absolute64_text_relocation(
                relocations,
                byte_offset + relocation_offset,
                selected_instruction_index,
                "in",
            )?;
        }
        CheckedInstructionValidationKind::MsrReadImmediateIndex {
            index,
            destination_byte_offset,
        } => {
            let mut prefix = Vec::with_capacity(27);
            prefix.extend([0x49, 0xba]);
            prefix.extend(u64::from(index).to_le_bytes());
            prefix.extend([
                0x44, 0x89, 0xd1, 0x0f, 0x32, 0x41, 0x89, 0xc2, 0x48, 0xc1, 0xe2, 0x20, 0x49, 0x09,
                0xd2, 0x49, 0xbf,
            ]);
            let mut suffix = Vec::with_capacity(7);
            suffix.extend([0x4d, 0x89, 0x97]);
            suffix.extend(destination_byte_offset.to_le_bytes());
            if encoded_bytes.len() != 42
                || !encoded_bytes.starts_with(&prefix)
                || encoded_bytes[27..35] != [0; 8]
                || !encoded_bytes.ends_with(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "encoded checked `rdmsr` instruction #{selected_instruction_index} does not bind index {index:#010x} and its destination through the closed result envelope"
                )));
            }
            if final_bytes.len() != 42
                || !final_bytes.starts_with(&prefix)
                || !final_bytes.ends_with(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "final checked `rdmsr` instruction #{selected_instruction_index} changed its index, privileged opcode, result combine, or destination envelope"
                )));
            }
            require_absolute64_text_relocation(
                relocations,
                byte_offset + 27,
                selected_instruction_index,
                "rdmsr",
            )?;
        }
        CheckedInstructionValidationKind::MsrWriteImmediateIndex {
            index,
            value_operand_byte_width,
        } => {
            let mut prefix = Vec::with_capacity(12);
            prefix.extend([0x49, 0xba]);
            prefix.extend(u64::from(index).to_le_bytes());
            prefix.extend([0x41, 0x52]);
            let suffix = [
                0x41, 0x5a, 0x44, 0x89, 0xd1, 0x44, 0x89, 0xd8, 0x4c, 0x89, 0xda, 0x48, 0xc1, 0xea,
                0x20, 0x0f, 0x30,
            ];
            let value_end = prefix
                .len()
                .checked_add(
                    usize::try_from(value_operand_byte_width)
                        .expect("u32 operand width fits usize"),
                )
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "checked `wrmsr` instruction #{selected_instruction_index} value width overflows"
                    ))
                })?;
            let expected_len = value_end.checked_add(suffix.len()).ok_or_else(|| {
                Diagnostic::error(format!(
                    "checked `wrmsr` instruction #{selected_instruction_index} width overflows"
                ))
            })?;
            if encoded_bytes.len() != expected_len
                || !encoded_bytes.starts_with(&prefix)
                || encoded_bytes.get(value_end..expected_len) != Some(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "encoded checked `wrmsr` instruction #{selected_instruction_index} does not bind index {index:#010x} through the closed split-value envelope"
                )));
            }
            if final_bytes.len() != expected_len
                || !final_bytes.starts_with(&prefix)
                || final_bytes.get(value_end..expected_len) != Some(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "final checked `wrmsr` instruction #{selected_instruction_index} changed its index or privileged opcode envelope"
                )));
            }
        }
        CheckedInstructionValidationKind::MsrReadRuntimeIndex {
            index_operand_byte_width,
            destination_byte_offset,
        } => {
            let index_end =
                usize::try_from(index_operand_byte_width).expect("u32 operand width fits usize");
            let relocation_offset = index_end.checked_add(17).ok_or_else(|| {
                Diagnostic::error(format!(
                    "checked `rdmsr` instruction #{selected_instruction_index} width overflows"
                ))
            })?;
            let expected_len = index_end.checked_add(32).ok_or_else(|| {
                Diagnostic::error(format!(
                    "checked `rdmsr` instruction #{selected_instruction_index} width overflows"
                ))
            })?;
            let fixed = [
                0x44, 0x89, 0xd1, 0x0f, 0x32, 0x41, 0x89, 0xc2, 0x48, 0xc1, 0xe2, 0x20, 0x49, 0x09,
                0xd2, 0x49, 0xbf,
            ];
            let mut suffix = Vec::with_capacity(7);
            suffix.extend([0x4d, 0x89, 0x97]);
            suffix.extend(destination_byte_offset.to_le_bytes());
            if encoded_bytes.len() != expected_len
                || encoded_bytes.get(index_end..index_end + fixed.len()) != Some(&fixed)
                || encoded_bytes.get(relocation_offset..relocation_offset + 8) != Some(&[0; 8])
                || !encoded_bytes.ends_with(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "encoded checked `rdmsr` instruction #{selected_instruction_index} does not preserve its runtime index boundary and closed result envelope"
                )));
            }
            if final_bytes.len() != expected_len
                || final_bytes.get(index_end..index_end + fixed.len()) != Some(&fixed)
                || !final_bytes.ends_with(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "final checked `rdmsr` instruction #{selected_instruction_index} changed its runtime index boundary, privileged opcode, result combine, or destination envelope"
                )));
            }
            require_absolute64_text_relocation(
                relocations,
                byte_offset + relocation_offset,
                selected_instruction_index,
                "rdmsr",
            )?;
        }
        CheckedInstructionValidationKind::MsrWriteRuntimeIndex {
            index_operand_byte_width,
            value_operand_byte_width,
        } => {
            let index_end =
                usize::try_from(index_operand_byte_width).expect("u32 operand width fits usize");
            let value_end = index_end
                .checked_add(2)
                .and_then(|start| {
                    start.checked_add(
                        usize::try_from(value_operand_byte_width)
                            .expect("u32 operand width fits usize"),
                    )
                })
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "checked `wrmsr` instruction #{selected_instruction_index} operand widths overflow"
                    ))
                })?;
            let suffix = [
                0x41, 0x5a, 0x44, 0x89, 0xd1, 0x44, 0x89, 0xd8, 0x4c, 0x89, 0xda, 0x48, 0xc1, 0xea,
                0x20, 0x0f, 0x30,
            ];
            let expected_len = value_end.checked_add(suffix.len()).ok_or_else(|| {
                Diagnostic::error(format!(
                    "checked `wrmsr` instruction #{selected_instruction_index} width overflows"
                ))
            })?;
            if encoded_bytes.len() != expected_len
                || encoded_bytes.get(index_end..index_end + 2) != Some(&[0x41, 0x52])
                || encoded_bytes.get(value_end..expected_len) != Some(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "encoded checked `wrmsr` instruction #{selected_instruction_index} does not preserve its runtime index/value boundaries and closed split-value envelope"
                )));
            }
            if final_bytes.len() != expected_len
                || final_bytes.get(index_end..index_end + 2) != Some(&[0x41, 0x52])
                || final_bytes.get(value_end..expected_len) != Some(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "final checked `wrmsr` instruction #{selected_instruction_index} changed its runtime operand boundaries or privileged opcode envelope"
                )));
            }
        }
        CheckedInstructionValidationKind::ControlRegisterRead {
            register,
            destination_byte_offset,
        } => {
            let modrm = control_register_modrm(register);
            let prefix = [0x41, 0x0f, 0x20, modrm, 0x49, 0xbf];
            let mut suffix = Vec::with_capacity(7);
            suffix.extend([0x4d, 0x89, 0x97]);
            suffix.extend(destination_byte_offset.to_le_bytes());
            if encoded_bytes.len() != 21
                || !encoded_bytes.starts_with(&prefix)
                || encoded_bytes[6..14] != [0; 8]
                || !encoded_bytes.ends_with(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "encoded checked control-register read #{selected_instruction_index} does not match its register and destination envelope"
                )));
            }
            if final_bytes.len() != 21
                || !final_bytes.starts_with(&prefix)
                || !final_bytes.ends_with(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "final checked control-register read #{selected_instruction_index} changed its register, privileged opcode, or destination envelope"
                )));
            }
            require_absolute64_text_relocation(
                relocations,
                byte_offset + 6,
                selected_instruction_index,
                register.read_mnemonic(),
            )?;
        }
        CheckedInstructionValidationKind::ControlRegisterWrite {
            register,
            source_operand_byte_width,
        } => {
            let suffix = [0x41, 0x0f, 0x22, control_register_modrm(register)];
            let source_end =
                usize::try_from(source_operand_byte_width).expect("u32 operand width fits usize");
            let expected_len = source_end.checked_add(suffix.len()).ok_or_else(|| {
                Diagnostic::error(format!(
                    "checked control-register write #{selected_instruction_index} width overflows"
                ))
            })?;
            if encoded_bytes.len() != expected_len
                || encoded_bytes.get(source_end..expected_len) != Some(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "encoded checked control-register write #{selected_instruction_index} does not match its register and privileged opcode envelope"
                )));
            }
            if final_bytes.len() != expected_len
                || final_bytes.get(source_end..expected_len) != Some(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "final checked control-register write #{selected_instruction_index} changed its register or privileged opcode envelope"
                )));
            }
        }
        CheckedInstructionValidationKind::FlagsSnapshot {
            destination_byte_offset,
        } => {
            let prefix = [0x9c, 0x41, 0x5a, 0x49, 0xbf];
            let mut suffix = Vec::with_capacity(7);
            suffix.extend([0x4d, 0x89, 0x97]);
            suffix.extend(destination_byte_offset.to_le_bytes());
            if encoded_bytes.len() != 20
                || !encoded_bytes.starts_with(&prefix)
                || encoded_bytes[5..13] != [0; 8]
                || !encoded_bytes.ends_with(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "encoded checked `pushfq` snapshot #{selected_instruction_index} does not match its balanced destination envelope"
                )));
            }
            if final_bytes.len() != 20
                || !final_bytes.starts_with(&prefix)
                || !final_bytes.ends_with(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "final checked `pushfq` snapshot #{selected_instruction_index} changed its flags operation or destination envelope"
                )));
            }
            require_absolute64_text_relocation(
                relocations,
                byte_offset + 5,
                selected_instruction_index,
                "pushfq",
            )?;
        }
        CheckedInstructionValidationKind::FlagsRestore {
            source_operand_byte_width,
        } => {
            let suffix = [0x41, 0x52, 0x9d];
            let source_end =
                usize::try_from(source_operand_byte_width).expect("u32 operand width fits usize");
            let expected_len = source_end.checked_add(suffix.len()).ok_or_else(|| {
                Diagnostic::error(format!(
                    "checked `popfq` restore #{selected_instruction_index} width overflows"
                ))
            })?;
            if encoded_bytes.len() != expected_len
                || encoded_bytes.get(source_end..expected_len) != Some(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "encoded checked `popfq` restore #{selected_instruction_index} does not match its balanced source envelope"
                )));
            }
            if final_bytes.len() != expected_len
                || final_bytes.get(source_end..expected_len) != Some(&suffix)
            {
                return Err(Diagnostic::error(format!(
                    "final checked `popfq` restore #{selected_instruction_index} changed its flags-restore envelope"
                )));
            }
        }
        _ => unreachable!("fixed checked instruction kinds returned above"),
    }
    Ok(())
}

fn require_absolute64_text_relocation(
    relocations: &RelocationPlan,
    expected_offset: usize,
    selected_instruction_index: u32,
    mnemonic: &str,
) -> Result<(), Diagnostic> {
    let matching_relocations = relocations
        .records()
        .filter(|(_, relocation)| {
            relocation.section == SectionKind::Text
                && relocation.kind == RelocationKind::Absolute64
                && relocation.offset == expected_offset
                && relocation.byte_width == 8
        })
        .count();
    if matching_relocations != 1 {
        return Err(Diagnostic::error(format!(
            "checked `{mnemonic}` instruction #{selected_instruction_index} requires exactly one destination relocation at final text byte {expected_offset}; found {matching_relocations}"
        )));
    }
    Ok(())
}

fn control_register_modrm(register: omega_core::inline_assembly::AsmControlRegister) -> u8 {
    use omega_core::inline_assembly::AsmControlRegister;
    match register {
        AsmControlRegister::Cr0 => 0xc2,
        AsmControlRegister::Cr2 => 0xd2,
        AsmControlRegister::Cr3 => 0xda,
        AsmControlRegister::Cr4 => 0xe2,
    }
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
        emit_checked_executable_image, validate_checked_instruction_bytes,
        validate_compiler_entry_call_return_bytes, validate_executable_region_enumeration,
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
    fn validates_checked_assembly_at_retained_instruction_boundaries() {
        use omega_core::arena::Arena;
        use omega_machine_bytes::{
            CheckedInstructionValidationKind, EncodedMachineCode, EncodedMachineInstruction,
        };

        let mut bytes = Arena::with_capacity(5);
        let halt = bytes.insert_many([0xf4]);
        let fence = bytes.insert_many([0x0f, 0xae, 0xf0]);
        let cli = bytes.insert_many([0xfa]);
        let mut instructions = Arena::with_capacity(3);
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 4,
            bytes: halt,
            checked_validation_kind: Some(CheckedInstructionValidationKind::MachineHalt),
            checked_operand_loaders: [None, None],
        });
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 5,
            bytes: fence,
            checked_validation_kind: Some(CheckedInstructionValidationKind::FullFence),
            checked_operand_loaders: [None, None],
        });
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 6,
            bytes: cli,
            checked_validation_kind: Some(CheckedInstructionValidationKind::InterruptDisable),
            checked_operand_loaders: [None, None],
        });
        let code = EncodedMachineCode {
            functions: Arena::new(),
            instructions,
            bytes,
            byte_count: 5,
        };

        let relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
        let (count, fingerprint) = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &[0xf4, 0x0f, 0xae, 0xf0, 0xfa],
            &relocations,
        )
        .expect("closed checked-assembly bytes should validate");
        assert_eq!(count, 3);
        assert_ne!(fingerprint, 0);

        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &[0xf4, 0x0f, 0xae, 0xe8, 0xfa],
            &relocations,
        )
        .expect_err("a changed final fence kind must reject");
        assert!(diagnostic.message.contains("changed after encoding"));
    }

    #[test]
    fn validates_immediate_port_identity_and_privileged_io_envelopes() {
        use omega_core::arena::Arena;
        use omega_machine_bytes::{
            CheckedInstructionValidationKind, CheckedOperandLoaderKind,
            CheckedOperandLoaderRegister, CheckedOperandLoaderValidation, EncodedMachineCode,
            EncodedMachineInstruction,
        };

        let mut out_bytes = Vec::new();
        out_bytes.extend([0x49, 0xba]);
        out_bytes.extend(0x3f8u64.to_le_bytes());
        out_bytes.extend([0x44, 0x89, 0xd2]);
        out_bytes.extend([0x49, 0xbb]);
        out_bytes.extend(0x41u64.to_le_bytes());
        out_bytes.extend([0x44, 0x89, 0xd8, 0xee]);
        let mut in_bytes = Vec::new();
        in_bytes.extend([0x49, 0xba]);
        in_bytes.extend(0x3fdu64.to_le_bytes());
        in_bytes.extend([0x44, 0x89, 0xd2, 0xec, 0x49, 0xbf]);
        in_bytes.extend(0u64.to_le_bytes());
        in_bytes.extend([0x41, 0x88, 0x87]);
        in_bytes.extend(4u32.to_le_bytes());

        let mut bytes = Arena::with_capacity(out_bytes.len() + in_bytes.len());
        let out_span = bytes.insert_many(out_bytes.iter().copied());
        let in_span = bytes.insert_many(in_bytes.iter().copied());
        let mut instructions = Arena::with_capacity(2);
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 8,
            bytes: out_span,
            checked_validation_kind: Some(
                CheckedInstructionValidationKind::PortWriteImmediatePort {
                    port: 0x3f8,
                    value_operand_byte_width: 10,
                },
            ),
            checked_operand_loaders: [
                Some(CheckedOperandLoaderValidation {
                    byte_offset: 0,
                    byte_width: 10,
                    register: CheckedOperandLoaderRegister::R10,
                    kind: CheckedOperandLoaderKind::Immediate { value: 0x3f8 },
                }),
                Some(CheckedOperandLoaderValidation {
                    byte_offset: 13,
                    byte_width: 10,
                    register: CheckedOperandLoaderRegister::R11,
                    kind: CheckedOperandLoaderKind::Immediate { value: 0x41 },
                }),
            ],
        });
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 9,
            bytes: in_span,
            checked_validation_kind: Some(
                CheckedInstructionValidationKind::PortReadImmediatePort {
                    port: 0x3fd,
                    destination_byte_offset: 4,
                },
            ),
            checked_operand_loaders: [
                Some(CheckedOperandLoaderValidation {
                    byte_offset: 0,
                    byte_width: 10,
                    register: CheckedOperandLoaderRegister::R10,
                    kind: CheckedOperandLoaderKind::Immediate { value: 0x3fd },
                }),
                None,
            ],
        });
        let code = EncodedMachineCode {
            functions: Arena::new(),
            instructions,
            bytes,
            byte_count: out_bytes.len() + in_bytes.len(),
        };
        let mut final_bytes = out_bytes;
        final_bytes.extend(in_bytes);
        let destination_relocation_offset = final_bytes.len() - 31 + 16;
        final_bytes[destination_relocation_offset..destination_relocation_offset + 8]
            .copy_from_slice(&0x1234_5678_9abc_def0u64.to_le_bytes());
        let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Instruction {
                function_symbol_handle: Handle::invalid(),
                selected_instruction_index: 9,
            },
            section: SectionKind::Text,
            offset: destination_relocation_offset,
            byte_width: 8,
            symbol_handle: Handle::invalid(),
            kind: RelocationKind::Absolute64,
        });

        let (count, fingerprint) = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &final_bytes,
            &relocations,
        )
        .expect("closed port identities and opcode envelopes should validate");
        assert_eq!(count, 2);
        assert_ne!(fingerprint, 0);

        let mut wrong_port = final_bytes.clone();
        wrong_port[2] ^= 1;
        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &wrong_port,
            &relocations,
        )
        .expect_err("changing a final port identity must reject");
        assert!(diagnostic.message.contains("changed its port"));

        let mut wrong_value = final_bytes.clone();
        wrong_value[15] ^= 1;
        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &wrong_value,
            &relocations,
        )
        .expect_err("changing a final immediate operand value must reject");
        assert!(diagnostic.message.contains("immediate operand loader"));

        let mut wrong_opcode = final_bytes;
        wrong_opcode[out_span.len() - 1] = 0x90;
        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &wrong_opcode,
            &relocations,
        )
        .expect_err("changing a final out opcode must reject");
        assert!(diagnostic.message.contains("privileged opcode envelope"));
    }

    #[test]
    fn validates_direct_storage_operand_loader_semantics() {
        use omega_core::arena::{Arena, Handle};
        use omega_core::inline_assembly::AsmControlRegister;
        use omega_machine_bytes::{
            CheckedInstructionValidationKind, CheckedOperandLoaderKind,
            CheckedOperandLoaderRegister, CheckedOperandLoaderValidation, EncodedMachineCode,
            EncodedMachineInstruction,
        };

        let mut encoded = Vec::new();
        encoded.extend([0x49, 0xbf]);
        encoded.extend(0u64.to_le_bytes());
        encoded.extend([0x4d, 0x8b, 0x97]);
        encoded.extend(32u32.to_le_bytes());
        encoded.extend([0x41, 0x0f, 0x22, 0xda]);

        let mut bytes = Arena::with_capacity(encoded.len());
        let span = bytes.insert_many(encoded.iter().copied());
        let mut instructions = Arena::with_capacity(1);
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 11,
            bytes: span,
            checked_validation_kind: Some(CheckedInstructionValidationKind::ControlRegisterWrite {
                register: AsmControlRegister::Cr3,
                source_operand_byte_width: 17,
            }),
            checked_operand_loaders: [
                Some(CheckedOperandLoaderValidation {
                    byte_offset: 0,
                    byte_width: 17,
                    register: CheckedOperandLoaderRegister::R10,
                    kind: CheckedOperandLoaderKind::Storage {
                        byte_offset: 32,
                        byte_size: 8,
                    },
                }),
                None,
            ],
        });
        let code = EncodedMachineCode {
            functions: Arena::new(),
            instructions,
            bytes,
            byte_count: encoded.len(),
        };

        let mut final_bytes = encoded;
        final_bytes[2..10].copy_from_slice(&0x1234_5678_9abc_def0u64.to_le_bytes());
        let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Instruction {
                function_symbol_handle: Handle::invalid(),
                selected_instruction_index: 11,
            },
            section: SectionKind::Text,
            offset: 2,
            byte_width: 8,
            symbol_handle: Handle::invalid(),
            kind: RelocationKind::Absolute64,
        });

        validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &final_bytes,
            &relocations,
        )
        .expect("direct storage loader semantics and relocation should validate");

        let mut wrong_load = final_bytes.clone();
        wrong_load[10] ^= 1;
        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &wrong_load,
            &relocations,
        )
        .expect_err("changing the retained source load must reject");
        assert!(diagnostic.message.contains("storage operand loader"));

        let missing_relocation = RelocationPlan::with_target(NativeTarget::linux_x64());
        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &final_bytes,
            &missing_relocation,
        )
        .expect_err("a storage loader without its exact relocation must reject");
        assert!(diagnostic.message.contains("source-storage relocation"));
    }

    fn indirect_operand_fixture(
        kind: omega_machine_bytes::CheckedOperandLoaderKind,
        pointer_byte_offset: u32,
        value_byte_offset: u32,
    ) -> (
        omega_machine_bytes::EncodedMachineCode,
        Vec<u8>,
        RelocationPlan,
    ) {
        use omega_core::arena::Arena;
        use omega_core::inline_assembly::AsmControlRegister;
        use omega_machine_bytes::{
            CheckedInstructionValidationKind, CheckedOperandLoaderRegister,
            CheckedOperandLoaderValidation, EncodedMachineCode, EncodedMachineInstruction,
        };

        let mut encoded = Vec::new();
        encoded.extend([0x49, 0xbf]);
        encoded.extend(0u64.to_le_bytes());
        encoded.extend([0x49, 0x8b, 0x87]);
        encoded.extend(pointer_byte_offset.to_le_bytes());
        encoded.extend([0x4c, 0x8b, 0x90]);
        encoded.extend(value_byte_offset.to_le_bytes());
        encoded.extend([0x41, 0x0f, 0x22, 0xda]);

        let mut bytes = Arena::with_capacity(encoded.len());
        let span = bytes.insert_many(encoded.iter().copied());
        let mut instructions = Arena::with_capacity(1);
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 12,
            bytes: span,
            checked_validation_kind: Some(CheckedInstructionValidationKind::ControlRegisterWrite {
                register: AsmControlRegister::Cr3,
                source_operand_byte_width: 24,
            }),
            checked_operand_loaders: [
                Some(CheckedOperandLoaderValidation {
                    byte_offset: 0,
                    byte_width: 24,
                    register: CheckedOperandLoaderRegister::R10,
                    kind,
                }),
                None,
            ],
        });
        let code = EncodedMachineCode {
            functions: Arena::new(),
            instructions,
            bytes,
            byte_count: encoded.len(),
        };

        let mut final_bytes = encoded;
        final_bytes[2..10].copy_from_slice(&0x1234_5678_9abc_def0u64.to_le_bytes());
        let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Instruction {
                function_symbol_handle: Handle::invalid(),
                selected_instruction_index: 12,
            },
            section: SectionKind::Text,
            offset: 2,
            byte_width: 8,
            symbol_handle: Handle::invalid(),
            kind: RelocationKind::Absolute64,
        });
        (code, final_bytes, relocations)
    }

    #[test]
    fn validates_pointee_and_fixed_index_operand_loader_semantics() {
        use omega_machine_bytes::CheckedOperandLoaderKind;

        let (pointee_code, pointee_bytes, pointee_relocations) = indirect_operand_fixture(
            CheckedOperandLoaderKind::Pointee {
                pointer_byte_offset: 24,
                field_byte_offset: 8,
                byte_size: 8,
            },
            24,
            8,
        );
        let (_, pointee_fingerprint) = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &pointee_code,
            &pointee_bytes,
            &pointee_relocations,
        )
        .expect("pointee loader semantics and relocation should validate");

        let (fixed_code, fixed_bytes, fixed_relocations) = indirect_operand_fixture(
            CheckedOperandLoaderKind::FrameFixedIndexed {
                descriptor_byte_offset: 24,
                element_index: 2,
                element_byte_size: 4,
                field_byte_offset: 0,
                byte_size: 8,
            },
            24,
            8,
        );
        let (_, fixed_fingerprint) = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &fixed_code,
            &fixed_bytes,
            &fixed_relocations,
        )
        .expect("fixed-index loader semantics and relocation should validate");
        assert_ne!(
            pointee_fingerprint, fixed_fingerprint,
            "semantically distinct operand plans must not share a certificate fingerprint"
        );

        let mut wrong_pointer_load = pointee_bytes;
        wrong_pointer_load[10] ^= 1;
        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &pointee_code,
            &wrong_pointer_load,
            &pointee_relocations,
        )
        .expect_err("changing the retained pointer load must reject");
        assert!(diagnostic.message.contains("indirect operand loader"));
    }

    #[test]
    fn validates_frame_base_indexed_operand_loader_semantics() {
        use omega_core::arena::Arena;
        use omega_core::inline_assembly::AsmControlRegister;
        use omega_machine_bytes::{
            CheckedInstructionValidationKind, CheckedOperandLoaderKind,
            CheckedOperandLoaderRegister, CheckedOperandLoaderValidation, EncodedMachineCode,
            EncodedMachineInstruction,
        };

        let mut encoded = Vec::new();
        encoded.extend([0x49, 0xbf]);
        encoded.extend(0u64.to_le_bytes());
        encoded.extend([0x45, 0x8b, 0x9f]);
        encoded.extend(16u32.to_le_bytes());
        encoded.extend([0x4d, 0x69, 0xdb]);
        encoded.extend(24u32.to_le_bytes());
        encoded.extend([0x4c, 0x89, 0xf8]);
        encoded.extend([0x4c, 0x01, 0xd8]);
        encoded.extend([0x4c, 0x8b, 0x90]);
        encoded.extend(40u32.to_le_bytes());
        encoded.extend([0x41, 0x0f, 0x22, 0xda]);

        let mut bytes = Arena::with_capacity(encoded.len());
        let span = bytes.insert_many(encoded.iter().copied());
        let mut instructions = Arena::with_capacity(1);
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 13,
            bytes: span,
            checked_validation_kind: Some(CheckedInstructionValidationKind::ControlRegisterWrite {
                register: AsmControlRegister::Cr3,
                source_operand_byte_width: 37,
            }),
            checked_operand_loaders: [
                Some(CheckedOperandLoaderValidation {
                    byte_offset: 0,
                    byte_width: 37,
                    register: CheckedOperandLoaderRegister::R10,
                    kind: CheckedOperandLoaderKind::FrameBaseIndexed {
                        base_byte_offset: 32,
                        index_byte_offset: 16,
                        index_byte_size: 4,
                        element_byte_size: 24,
                        field_byte_offset: 8,
                        byte_size: 8,
                    },
                }),
                None,
            ],
        });
        let code = EncodedMachineCode {
            functions: Arena::new(),
            instructions,
            bytes,
            byte_count: encoded.len(),
        };

        let mut final_bytes = encoded;
        final_bytes[2..10].copy_from_slice(&0x1234_5678_9abc_def0u64.to_le_bytes());
        let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
        relocations.push_record(RelocationRecord {
            origin: RelocationOrigin::Instruction {
                function_symbol_handle: Handle::invalid(),
                selected_instruction_index: 13,
            },
            section: SectionKind::Text,
            offset: 2,
            byte_width: 8,
            symbol_handle: Handle::invalid(),
            kind: RelocationKind::Absolute64,
        });

        validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &final_bytes,
            &relocations,
        )
        .expect("frame-base-indexed loader semantics and relocation should validate");

        let mut wrong_scale = final_bytes;
        wrong_scale[20] ^= 1;
        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &wrong_scale,
            &relocations,
        )
        .expect_err("changing the retained element scale must reject");
        assert!(
            diagnostic
                .message
                .contains("frame-base-indexed operand loader")
        );
    }

    #[test]
    fn validates_cross_region_frame_indexed_operand_loader_semantics() {
        use omega_core::arena::Arena;
        use omega_core::inline_assembly::AsmControlRegister;
        use omega_machine_bytes::{
            CheckedInstructionValidationKind, CheckedOperandLoaderKind,
            CheckedOperandLoaderRegister, CheckedOperandLoaderValidation, EncodedMachineCode,
            EncodedMachineInstruction,
        };

        let mut encoded = Vec::new();
        encoded.extend([0x49, 0xbf]);
        encoded.extend(0u64.to_le_bytes());
        encoded.extend([0x49, 0x8b, 0x87]);
        encoded.extend(24u32.to_le_bytes());
        encoded.extend([0x49, 0xbf]);
        encoded.extend(0u64.to_le_bytes());
        encoded.extend([0x45, 0x0f, 0xb6, 0x9f]);
        encoded.extend(12u32.to_le_bytes());
        encoded.extend([0x4d, 0x69, 0xdb]);
        encoded.extend(32u32.to_le_bytes());
        encoded.extend([0x4c, 0x01, 0xd8]);
        encoded.extend([0x4c, 0x8b, 0x90]);
        encoded.extend(8u32.to_le_bytes());
        encoded.extend([0x41, 0x0f, 0x22, 0xda]);

        let mut bytes = Arena::with_capacity(encoded.len());
        let span = bytes.insert_many(encoded.iter().copied());
        let mut instructions = Arena::with_capacity(1);
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 14,
            bytes: span,
            checked_validation_kind: Some(CheckedInstructionValidationKind::ControlRegisterWrite {
                register: AsmControlRegister::Cr3,
                source_operand_byte_width: 52,
            }),
            checked_operand_loaders: [
                Some(CheckedOperandLoaderValidation {
                    byte_offset: 0,
                    byte_width: 52,
                    register: CheckedOperandLoaderRegister::R10,
                    kind: CheckedOperandLoaderKind::FrameIndexed {
                        descriptor_byte_offset: 24,
                        index_from_machine: true,
                        index_byte_offset: 12,
                        index_byte_size: 1,
                        element_byte_size: 32,
                        field_byte_offset: 8,
                        byte_size: 8,
                    },
                }),
                None,
            ],
        });
        let code = EncodedMachineCode {
            functions: Arena::new(),
            instructions,
            bytes,
            byte_count: encoded.len(),
        };

        let mut final_bytes = encoded;
        final_bytes[2..10].copy_from_slice(&0x1234_5678_9abc_def0u64.to_le_bytes());
        final_bytes[19..27].copy_from_slice(&0x0fed_cba9_8765_4321u64.to_le_bytes());
        let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
        for offset in [2, 19] {
            relocations.push_record(RelocationRecord {
                origin: RelocationOrigin::Instruction {
                    function_symbol_handle: Handle::invalid(),
                    selected_instruction_index: 14,
                },
                section: SectionKind::Text,
                offset,
                byte_width: 8,
                symbol_handle: Handle::invalid(),
                kind: RelocationKind::Absolute64,
            });
        }

        validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &final_bytes,
            &relocations,
        )
        .expect("cross-region frame-indexed semantics and both relocations should validate");

        let mut missing_second = RelocationPlan::with_target(NativeTarget::linux_x64());
        missing_second.push_record(RelocationRecord {
            origin: RelocationOrigin::Instruction {
                function_symbol_handle: Handle::invalid(),
                selected_instruction_index: 14,
            },
            section: SectionKind::Text,
            offset: 2,
            byte_width: 8,
            symbol_handle: Handle::invalid(),
            kind: RelocationKind::Absolute64,
        });
        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &final_bytes,
            &missing_second,
        )
        .expect_err("a cross-region operand without its index-base relocation must reject");
        assert!(diagnostic.message.contains("source-storage relocation"));
    }

    #[test]
    fn rejects_mutated_final_wrmsr_opcode_after_index_binding() {
        use omega_core::arena::Arena;
        use omega_machine_bytes::{
            CheckedInstructionValidationKind, CheckedOperandLoaderKind,
            CheckedOperandLoaderRegister, CheckedOperandLoaderValidation, EncodedMachineCode,
            EncodedMachineInstruction,
        };

        let mut encoded = Vec::new();
        encoded.extend([0x49, 0xba]);
        encoded.extend(0xc000_0080u64.to_le_bytes());
        encoded.extend([0x41, 0x52]);
        encoded.extend([0x49, 0xbb]);
        encoded.extend(0x1122_3344_5566_7788u64.to_le_bytes());
        encoded.extend([
            0x41, 0x5a, 0x44, 0x89, 0xd1, 0x44, 0x89, 0xd8, 0x4c, 0x89, 0xda, 0x48, 0xc1, 0xea,
            0x20, 0x0f, 0x30,
        ]);
        let mut bytes = Arena::with_capacity(encoded.len());
        let span = bytes.insert_many(encoded.iter().copied());
        let mut instructions = Arena::with_capacity(1);
        instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: 10,
            bytes: span,
            checked_validation_kind: Some(
                CheckedInstructionValidationKind::MsrWriteImmediateIndex {
                    index: 0xc000_0080,
                    value_operand_byte_width: 10,
                },
            ),
            checked_operand_loaders: [
                Some(CheckedOperandLoaderValidation {
                    byte_offset: 0,
                    byte_width: 10,
                    register: CheckedOperandLoaderRegister::R10,
                    kind: CheckedOperandLoaderKind::Immediate { value: 0xc000_0080 },
                }),
                Some(CheckedOperandLoaderValidation {
                    byte_offset: 12,
                    byte_width: 10,
                    register: CheckedOperandLoaderRegister::R11,
                    kind: CheckedOperandLoaderKind::Immediate {
                        value: 0x1122_3344_5566_7788,
                    },
                }),
            ],
        });
        let code = EncodedMachineCode {
            functions: Arena::new(),
            instructions,
            bytes,
            byte_count: encoded.len(),
        };
        let relocations = RelocationPlan::with_target(NativeTarget::linux_x64());

        validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &encoded,
            &relocations,
        )
        .expect("exact WRMSR index and split-value envelope should validate");

        let last = encoded.len() - 1;
        encoded[last] = 0x31;
        let diagnostic = validate_checked_instruction_bytes(
            omega_target::Architecture::X86_64,
            &code,
            &encoded,
            &relocations,
        )
        .expect_err("a changed final WRMSR opcode must reject");
        assert!(diagnostic.message.contains("privileged opcode envelope"));
    }
}
