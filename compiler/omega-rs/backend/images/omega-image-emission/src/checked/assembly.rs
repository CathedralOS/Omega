//! Validates checked-assembly footprints, operand loaders, and exact instruction bytes.

use super::*;

fn checked_assembly_instruction_footprint(
    instruction: &omega_machine_bytes::EncodedMachineInstruction,
) -> omega_calling_conventions::StateFootprintEvidence {
    use omega_calling_conventions::{
        MachineRegister, MachineState, MachineStateSet, RegisterSet, StateFootprintEvidence,
    };
    use omega_machine_bytes::CheckedInstructionValidationKind as Kind;

    let kind = instruction
        .checked_validation_kind
        .expect("checked footprint rows have a validation kind");
    let (registers, fixed_state) = match kind {
        Kind::MachineHalt => (
            RegisterSet::default(),
            MachineStateSet::new([MachineState::InstructionPointer, MachineState::ControlState]),
        ),
        Kind::LoadFence | Kind::StoreFence | Kind::FullFence => {
            (RegisterSet::default(), MachineStateSet::empty())
        }
        Kind::InterruptDisable | Kind::InterruptEnable => (
            RegisterSet::default(),
            MachineStateSet::new([MachineState::Flags]),
        ),
        Kind::PortWriteImmediatePort { .. } | Kind::PortWriteRuntimePort { .. } => (
            RegisterSet::new([
                MachineRegister::X86Rax,
                MachineRegister::X86Rdx,
                MachineRegister::X86R10,
                MachineRegister::X86R11,
                MachineRegister::X86R15,
            ]),
            MachineStateSet::empty(),
        ),
        Kind::PortReadImmediatePort { .. } | Kind::PortReadRuntimePort { .. } => (
            RegisterSet::new([
                MachineRegister::X86Rax,
                MachineRegister::X86Rdx,
                MachineRegister::X86R10,
                MachineRegister::X86R15,
            ]),
            MachineStateSet::empty(),
        ),
        Kind::FlagsSnapshot { .. } => (
            RegisterSet::new([MachineRegister::X86R10, MachineRegister::X86R15]),
            MachineStateSet::new([MachineState::StackPointer]),
        ),
        Kind::FlagsRestore { .. } => (
            RegisterSet::new([MachineRegister::X86R10, MachineRegister::X86R15]),
            MachineStateSet::new([MachineState::Flags, MachineState::StackPointer]),
        ),
        Kind::MsrReadImmediateIndex { .. } | Kind::MsrReadRuntimeIndex { .. } => (
            RegisterSet::new([
                MachineRegister::X86Rax,
                MachineRegister::X86Rcx,
                MachineRegister::X86Rdx,
                MachineRegister::X86R10,
                MachineRegister::X86R11,
                MachineRegister::X86R15,
            ]),
            MachineStateSet::new([MachineState::Flags]),
        ),
        Kind::MsrWriteImmediateIndex { .. } | Kind::MsrWriteRuntimeIndex { .. } => (
            RegisterSet::new([
                MachineRegister::X86Rax,
                MachineRegister::X86Rcx,
                MachineRegister::X86Rdx,
                MachineRegister::X86R10,
                MachineRegister::X86R11,
                MachineRegister::X86R15,
            ]),
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::StackPointer,
                MachineState::ControlState,
            ]),
        ),
        Kind::ControlRegisterRead { .. } => (
            RegisterSet::new([MachineRegister::X86R10, MachineRegister::X86R15]),
            MachineStateSet::empty(),
        ),
        Kind::ControlRegisterWrite { .. } => (
            RegisterSet::new([
                MachineRegister::X86Rax,
                MachineRegister::X86R10,
                MachineRegister::X86R11,
                MachineRegister::X86R15,
            ]),
            MachineStateSet::new([MachineState::ControlState]),
        ),
    };
    let loader_state = instruction
        .checked_operand_loaders
        .into_iter()
        .flatten()
        .fold(MachineStateSet::empty(), |state, loader| {
            state.union(checked_operand_loader_footprint(loader))
        });
    StateFootprintEvidence::new(registers, fixed_state.union(loader_state))
}

fn checked_operand_loader_footprint(
    loader: omega_machine_bytes::CheckedOperandLoaderValidation,
) -> omega_calling_conventions::MachineStateSet {
    use omega_calling_conventions::{MachineState, MachineStateSet};
    use omega_machine_bytes::CheckedOperandLoaderKind as Kind;

    match loader.kind {
        Kind::Pointee {
            field_byte_offset, ..
        } if field_byte_offset != 0 => MachineStateSet::new([MachineState::Flags]),
        Kind::FrameBaseIndexed { .. } | Kind::FrameIndexed { .. } | Kind::MachineIndexed { .. } => {
            MachineStateSet::new([MachineState::Flags])
        }
        Kind::Immediate { .. }
        | Kind::Storage { .. }
        | Kind::Pointee { .. }
        | Kind::FrameFixedIndexed { .. } => MachineStateSet::empty(),
    }
}

pub(super) fn validate_checked_assembly_footprint(
    code: &omega_machine_bytes::EncodedMachineCode,
    semantics: &omega_machine_bytes::EncodedMachineSemanticSummary,
) -> Result<u64, Diagnostic> {
    use omega_machine_instructions::BoundaryFootprintFragmentOrigin;

    let derived = code
        .instructions
        .iter()
        .filter_map(|(_, instruction)| instruction.checked_validation_kind.map(|_| instruction))
        .map(checked_assembly_instruction_footprint)
        .collect::<Vec<_>>();
    let composed = omega_calling_conventions::compose_state_footprints(derived.iter());
    let retained = semantics
        .boundaries
        .footprints
        .fragments
        .iter()
        .filter(|fragment| {
            fragment.origin == BoundaryFootprintFragmentOrigin::CheckedAssemblyCatalog
        })
        .collect::<Vec<_>>();
    let composed_is_empty =
        composed.registers().as_slice().is_empty() && composed.machine_state().is_empty();
    if composed_is_empty {
        if !retained.is_empty() {
            return Err(Diagnostic::error(
                "retained checked-assembly footprint has no final catalog instruction rows",
            ));
        }
    } else if retained.len() != 1 || retained[0].evidence != composed {
        return Err(Diagnostic::error(format!(
            "final checked-assembly catalog footprint does not match its StatePlan-validated semantic fragment: retained={:?}, replayed={composed:?}",
            retained
                .iter()
                .map(|fragment| &fragment.evidence)
                .collect::<Vec<_>>()
        )));
    }
    Ok(composed.evidence_fingerprint())
}

pub(super) fn validate_checked_instruction_bytes(
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
        let expected_loaders = match kind {
            CheckedInstructionValidationKind::PortWriteImmediatePort { .. }
            | CheckedInstructionValidationKind::PortWriteRuntimePort { .. }
            | CheckedInstructionValidationKind::MsrWriteImmediateIndex { .. }
            | CheckedInstructionValidationKind::MsrWriteRuntimeIndex { .. } => [true, true],
            CheckedInstructionValidationKind::PortReadImmediatePort { .. }
            | CheckedInstructionValidationKind::PortReadRuntimePort { .. }
            | CheckedInstructionValidationKind::MsrReadImmediateIndex { .. }
            | CheckedInstructionValidationKind::MsrReadRuntimeIndex { .. }
            | CheckedInstructionValidationKind::ControlRegisterWrite { .. }
            | CheckedInstructionValidationKind::FlagsRestore { .. } => [true, false],
            CheckedInstructionValidationKind::MachineHalt
            | CheckedInstructionValidationKind::LoadFence
            | CheckedInstructionValidationKind::StoreFence
            | CheckedInstructionValidationKind::FullFence
            | CheckedInstructionValidationKind::InterruptDisable
            | CheckedInstructionValidationKind::InterruptEnable
            | CheckedInstructionValidationKind::ControlRegisterRead { .. }
            | CheckedInstructionValidationKind::FlagsSnapshot { .. } => [false, false],
        };
        let retained_loaders = instruction
            .checked_operand_loaders
            .map(|loader| loader.is_some());
        if retained_loaders != expected_loaders {
            return Err(Diagnostic::error(format!(
                "checked-assembly instruction #{} does not retain its complete operand-loader validation envelope",
                instruction.selected_instruction_index
            )));
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
        Kind::MachineIndexed {
            base_byte_offset,
            index_from_frame,
            index_byte_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => {
            fingerprint_into(
                fingerprint,
                &[7, u8::from(index_from_frame), index_byte_size, byte_size],
            );
            fingerprint_into(fingerprint, &base_byte_offset.to_le_bytes());
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
        Kind::MachineIndexed {
            base_byte_offset,
            index_from_frame,
            index_byte_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => {
            validate_checked_machine_indexed_operand_loader(
                loader.register,
                base_byte_offset,
                index_from_frame,
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
            if index_from_frame {
                require_checked_operand_storage_relocation(
                    relocations,
                    instruction_byte_offset + start + 13 + 2,
                    selected_instruction_index,
                )?;
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_checked_machine_indexed_operand_loader(
    register: omega_machine_bytes::CheckedOperandLoaderRegister,
    base_byte_offset: u32,
    index_from_frame: bool,
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
            "checked-assembly instruction #{selected_instruction_index} machine-indexed index displacement does not fit x86 disp32"
        ))
    })?;
    let element_scale = i32::try_from(element_byte_size).map_err(|_| {
        Diagnostic::error(format!(
            "checked-assembly instruction #{selected_instruction_index} machine-indexed element scale does not fit x86 imm32"
        ))
    })?;
    let value_byte_offset = base_byte_offset
        .checked_add(field_byte_offset)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "checked-assembly instruction #{selected_instruction_index} machine-indexed value displacement overflows its retained range"
            ))
        })?;
    let value_displacement = i32::try_from(value_byte_offset).map_err(|_| {
        Diagnostic::error(format!(
            "checked-assembly instruction #{selected_instruction_index} machine-indexed value displacement does not fit x86 disp32"
        ))
    })?;
    let index_opcode: &[u8] = match (index_from_frame, index_byte_size) {
        (true, 1) => &[0x45, 0x0f, 0xb6, 0x9f],
        (true, 2) => &[0x45, 0x0f, 0xb7, 0x9f],
        (true, 4) => &[0x45, 0x8b, 0x9f],
        (true, 8) => &[0x4d, 0x8b, 0x9f],
        (false, 1) => &[0x44, 0x0f, 0xb6, 0x98],
        (false, 2) => &[0x44, 0x0f, 0xb7, 0x98],
        (false, 4) => &[0x44, 0x8b, 0x98],
        (false, 8) => &[0x4c, 0x8b, 0x98],
        _ => {
            return Err(Diagnostic::error(format!(
                "checked-assembly instruction #{selected_instruction_index} retains unsupported {index_byte_size}-byte machine-index semantics"
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
                "checked-assembly instruction #{selected_instruction_index} retains unsupported {byte_size}-byte machine-indexed value semantics"
            )));
        }
    };
    let mut expected = Vec::with_capacity(width);
    expected.extend([0x49, 0xbf]);
    expected.extend(0u64.to_le_bytes());
    expected.extend([0x4c, 0x89, 0xf8]);
    if index_from_frame {
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
            "encoded checked-assembly instruction #{selected_instruction_index} machine-indexed operand loader does not match its retained base/index/scale/value semantics"
        )));
    }
    let mut expected_final = expected;
    expected_final[2..10].copy_from_slice(&final_bytes[2..10]);
    if index_from_frame {
        expected_final[15..23].copy_from_slice(&final_bytes[15..23]);
    }
    if final_bytes != expected_final {
        return Err(Diagnostic::error(format!(
            "final checked-assembly instruction #{selected_instruction_index} changed its machine-indexed operand loader semantics"
        )));
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
                && relocation.addend == 0
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
