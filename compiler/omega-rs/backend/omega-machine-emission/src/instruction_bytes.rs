use crate::MachineEmissionContext;
use crate::branch_distances;
use crate::encoding::encode_machine_instruction_bytes;
use crate::layout::{self, layout_machine_instructions};
use omega_assigned_target_operations::{
    SelectedInstructionKind, StateGuardLowering, StateGuardOperator, TargetOperationKind,
};
use omega_machine_bytes::{
    CheckedInstructionValidationKind, CheckedOperandLoaderKind, CheckedOperandLoaderRegister,
    CheckedOperandLoaderValidation, EncodedMachineCode, EncodedMachineInstruction,
};
use omega_machine_instructions::{MachineInstruction, MachineInstructionPlan};
use omega_target_operations::RuntimeValueOperandSource;
use psi_arena::{Arena, HandleSpan};
use psi_diagnostics::Diagnostic;

pub(crate) fn emit_function_bytes(
    emission_context: MachineEmissionContext<'_>,
    machine_instructions: &MachineInstructionPlan,
    encoded_code: &mut EncodedMachineCode,
    machine_instructions_span: HandleSpan<MachineInstruction>,
) -> Result<(), Diagnostic> {
    let Some(machine_instructions) = machine_instructions
        .code
        .instructions
        .span(machine_instructions_span)
    else {
        return Ok(());
    };
    let laid_out_instructions =
        layout_machine_instructions(emission_context, machine_instructions)?;
    encoded_code.bytes.reserve(
        laid_out_instructions
            .iter()
            .map(|instruction| instruction.byte_width)
            .sum(),
    );

    for (machine_instruction_index, machine_instruction) in machine_instructions.iter().enumerate()
    {
        let laid_out_instruction = &laid_out_instructions[machine_instruction_index];
        // A runtime-VALUE guard comparison reaching emission means its right
        // operand never resolved to storage (e.g. an unrecognized member
        // accessor, like a carrier `.len` before the resolver was taught it). It
        // has no encoder here, so it would encode to ZERO bytes and the guard
        // would be SILENTLY DROPPED -- the `true` arm taken unconditionally. A
        // resolved runtime comparison lowers to a `CompareRuntimeStorage`
        // instruction, so this kind must never reach emission; refuse rather
        // than miscompile.
        if matches!(
            &machine_instruction.source_kind,
            SelectedInstructionKind::EvaluateDispatchGuard {
                guard_lowering: StateGuardLowering::CompareRuntimeValue,
                ..
            }
        ) {
            return Err(Diagnostic::error(
                "dispatch guard runtime comparison operand did not resolve to storage; \
                 the guard cannot be emitted (it would be silently dropped, taking the \
                 true arm unconditionally)",
            ));
        }
        if laid_out_instruction.byte_width == 0 {
            if machine_instruction
                .kind
                .requires_checked_assembly_validation()
            {
                return Err(Diagnostic::error(format!(
                    "checked-assembly instruction #{} reached emission without bytes",
                    machine_instruction.selected_instruction_index
                )));
            }
            encoded_code.instructions.insert(EncodedMachineInstruction {
                selected_instruction_index: machine_instruction.selected_instruction_index,
                bytes: HandleSpan::empty(),
                checked_validation_kind: None,
                checked_operand_loaders: [None, None],
            });
            continue;
        }

        let byte_span = insert_encoded_machine_instruction(
            &mut encoded_code.bytes,
            emission_context,
            &laid_out_instructions,
            machine_instruction_index,
            &machine_instruction.source_kind,
        )?;
        if byte_span.len() != laid_out_instruction.byte_width {
            let operand_note = match &machine_instruction.source_kind {
                SelectedInstructionKind::WritePlaceBinary { left, right, .. } => {
                    format!(
                        "; operands: left={:?}, right={:?}",
                        emission_context
                            .assigned_target_operations
                            .runtime_value_operand(*left)
                            .expect("assigned left runtime value operand should exist"),
                        emission_context
                            .assigned_target_operations
                            .runtime_value_operand(*right)
                            .expect("assigned right runtime value operand should exist"),
                    )
                }
                _ => String::new(),
            };
            return Err(Diagnostic::error(format!(
                "encoded instruction width mismatch for selected #{} ({:?} from {:?}): layout planned {} byte(s), encoder emitted {} byte(s){}",
                machine_instruction.selected_instruction_index,
                machine_instruction.source_kind,
                machine_instruction.kind,
                laid_out_instruction.byte_width,
                byte_span.len(),
                operand_note,
            )));
        }
        let checked_validation_kind =
            checked_instruction_validation_kind(emission_context, &machine_instruction.source_kind);
        let checked_operand_loaders =
            checked_operand_loaders(emission_context, &machine_instruction.source_kind);
        if machine_instruction
            .kind
            .requires_checked_assembly_validation()
            && checked_validation_kind.is_none()
        {
            return Err(Diagnostic::error(format!(
                "checked-assembly instruction #{} reached emission without final-image validation evidence",
                machine_instruction.selected_instruction_index
            )));
        }
        encoded_code.instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: machine_instruction.selected_instruction_index,
            bytes: byte_span,
            checked_validation_kind,
            checked_operand_loaders,
        });
    }

    Ok(())
}

fn checked_operand_loader(
    emission_context: MachineEmissionContext<'_>,
    operand: omega_target_operations::RuntimeValueOperandHandle,
    byte_offset: usize,
    register: CheckedOperandLoaderRegister,
) -> Option<CheckedOperandLoaderValidation> {
    let source = emission_context.assigned_target_operations;
    let byte_width = omega_instruction_selection::runtime_value_operand_width(
        omega_target::Architecture::X86_64,
        source,
        operand,
    );
    let kind = if let Some(value) = source.immediate_integer(operand) {
        CheckedOperandLoaderKind::Immediate {
            value: value as u64,
        }
    } else if let Some((_region, storage_offset, byte_size)) = source.storage(operand) {
        CheckedOperandLoaderKind::Storage {
            byte_offset: u32::try_from(storage_offset).ok()?,
            byte_size: u8::try_from(byte_size).ok()?,
        }
    } else if let Some((pointer_byte_offset, field_byte_offset, byte_size)) =
        source.pointee(operand)
    {
        CheckedOperandLoaderKind::Pointee {
            pointer_byte_offset: u32::try_from(pointer_byte_offset).ok()?,
            field_byte_offset: u32::try_from(field_byte_offset).ok()?,
            byte_size: u8::try_from(byte_size).ok()?,
        }
    } else if let Some((
        descriptor_byte_offset,
        element_index,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )) = source.frame_fixed_indexed(operand)
    {
        CheckedOperandLoaderKind::FrameFixedIndexed {
            descriptor_byte_offset: u32::try_from(descriptor_byte_offset).ok()?,
            element_index: u64::try_from(element_index).ok()?,
            element_byte_size: u32::try_from(element_byte_size).ok()?,
            field_byte_offset: u32::try_from(field_byte_offset).ok()?,
            byte_size: u8::try_from(byte_size).ok()?,
        }
    } else if let Some((
        base_byte_offset,
        index_byte_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )) = source.frame_base_indexed(operand)
    {
        CheckedOperandLoaderKind::FrameBaseIndexed {
            base_byte_offset: u32::try_from(base_byte_offset).ok()?,
            index_byte_offset: u32::try_from(index_byte_offset).ok()?,
            index_byte_size: u8::try_from(index_byte_size).ok()?,
            element_byte_size: u32::try_from(element_byte_size).ok()?,
            field_byte_offset: u32::try_from(field_byte_offset).ok()?,
            byte_size: u8::try_from(byte_size).ok()?,
        }
    } else if let Some((
        descriptor_byte_offset,
        index_region,
        index_byte_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )) = source.frame_indexed(operand)
    {
        CheckedOperandLoaderKind::FrameIndexed {
            descriptor_byte_offset: u32::try_from(descriptor_byte_offset).ok()?,
            index_from_machine: index_region
                == omega_target_operations::RuntimeStorageRegion::Machine,
            index_byte_offset: u32::try_from(index_byte_offset).ok()?,
            index_byte_size: u8::try_from(index_byte_size).ok()?,
            element_byte_size: u32::try_from(element_byte_size).ok()?,
            field_byte_offset: u32::try_from(field_byte_offset).ok()?,
            byte_size: u8::try_from(byte_size).ok()?,
        }
    } else if let Some((
        base_byte_offset,
        index_region,
        index_byte_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )) = source.machine_indexed(operand)
    {
        CheckedOperandLoaderKind::MachineIndexed {
            base_byte_offset: u32::try_from(base_byte_offset).ok()?,
            index_from_frame: index_region
                == omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
            index_byte_offset: u32::try_from(index_byte_offset).ok()?,
            index_byte_size: u8::try_from(index_byte_size).ok()?,
            element_byte_size: u32::try_from(element_byte_size).ok()?,
            field_byte_offset: u32::try_from(field_byte_offset).ok()?,
            byte_size: u8::try_from(byte_size).ok()?,
        }
    } else {
        return None;
    };
    Some(CheckedOperandLoaderValidation {
        byte_offset: u32::try_from(byte_offset).ok()?,
        byte_width: u32::try_from(byte_width).ok()?,
        register,
        kind,
    })
}

fn checked_operand_loaders(
    emission_context: MachineEmissionContext<'_>,
    kind: &SelectedInstructionKind,
) -> [Option<CheckedOperandLoaderValidation>; 2] {
    use CheckedOperandLoaderRegister::{R10, R11};

    let mut loaders = [None, None];
    match kind {
        SelectedInstructionKind::PortWrite { port, value } => {
            let port_width = omega_instruction_selection::runtime_value_operand_width(
                omega_target::Architecture::X86_64,
                emission_context.assigned_target_operations,
                *port,
            );
            loaders[0] = checked_operand_loader(emission_context, *port, 0, R10);
            loaders[1] = checked_operand_loader(
                emission_context,
                *value,
                port_width + omega_isa_x86_64::PORT_OPERAND_REGISTER_MOVE_WIDTH,
                R11,
            );
        }
        SelectedInstructionKind::PortRead { port, .. } => {
            loaders[0] = checked_operand_loader(emission_context, *port, 0, R10);
        }
        SelectedInstructionKind::MsrRead { index, .. } => {
            loaders[0] = checked_operand_loader(emission_context, *index, 0, R10);
        }
        SelectedInstructionKind::MsrWrite { index, value } => {
            let index_width = omega_instruction_selection::runtime_value_operand_width(
                omega_target::Architecture::X86_64,
                emission_context.assigned_target_operations,
                *index,
            );
            loaders[0] = checked_operand_loader(emission_context, *index, 0, R10);
            loaders[1] = checked_operand_loader(emission_context, *value, index_width + 2, R11);
        }
        SelectedInstructionKind::ControlRegisterWrite { source, .. }
        | SelectedInstructionKind::FlagsRestore { source } => {
            loaders[0] = checked_operand_loader(emission_context, *source, 0, R10);
        }
        _ => {}
    }
    loaders
}

fn checked_instruction_validation_kind(
    emission_context: MachineEmissionContext<'_>,
    kind: &SelectedInstructionKind,
) -> Option<CheckedInstructionValidationKind> {
    use psi_language_core::inline_assembly::{AsmFenceKind, AsmInterruptControlKind};

    match kind {
        SelectedInstructionKind::MachineHalt => Some(CheckedInstructionValidationKind::MachineHalt),
        SelectedInstructionKind::MemoryFence(AsmFenceKind::Load) => {
            Some(CheckedInstructionValidationKind::LoadFence)
        }
        SelectedInstructionKind::MemoryFence(AsmFenceKind::Store) => {
            Some(CheckedInstructionValidationKind::StoreFence)
        }
        SelectedInstructionKind::MemoryFence(AsmFenceKind::Full) => {
            Some(CheckedInstructionValidationKind::FullFence)
        }
        SelectedInstructionKind::InterruptControl(AsmInterruptControlKind::Disable) => {
            Some(CheckedInstructionValidationKind::InterruptDisable)
        }
        SelectedInstructionKind::InterruptControl(AsmInterruptControlKind::Enable) => {
            Some(CheckedInstructionValidationKind::InterruptEnable)
        }
        SelectedInstructionKind::PortWrite { port, value } => {
            let value_operand_byte_width =
                u32::try_from(omega_instruction_selection::runtime_value_operand_width(
                    omega_target::Architecture::X86_64,
                    emission_context.assigned_target_operations,
                    *value,
                ))
                .ok()?;
            if let Some(port) = emission_context
                .assigned_target_operations
                .immediate_integer(*port)
                .and_then(|port| u16::try_from(port).ok())
            {
                Some(CheckedInstructionValidationKind::PortWriteImmediatePort {
                    port,
                    value_operand_byte_width,
                })
            } else {
                let port_operand_byte_width =
                    u32::try_from(omega_instruction_selection::runtime_value_operand_width(
                        omega_target::Architecture::X86_64,
                        emission_context.assigned_target_operations,
                        *port,
                    ))
                    .ok()?;
                Some(CheckedInstructionValidationKind::PortWriteRuntimePort {
                    port_operand_byte_width,
                    value_operand_byte_width,
                })
            }
        }
        SelectedInstructionKind::PortRead {
            port,
            dest_byte_offset,
            ..
        } => {
            let port_value = emission_context
                .assigned_target_operations
                .immediate_integer(*port)
                .and_then(|port| u16::try_from(port).ok());
            let destination_byte_offset = u32::try_from(*dest_byte_offset).ok()?;
            if let Some(port) = port_value {
                Some(CheckedInstructionValidationKind::PortReadImmediatePort {
                    port,
                    destination_byte_offset,
                })
            } else {
                let port_operand_byte_width =
                    u32::try_from(omega_instruction_selection::runtime_value_operand_width(
                        omega_target::Architecture::X86_64,
                        emission_context.assigned_target_operations,
                        *port,
                    ))
                    .ok()?;
                Some(CheckedInstructionValidationKind::PortReadRuntimePort {
                    port_operand_byte_width,
                    destination_byte_offset,
                })
            }
        }
        SelectedInstructionKind::MsrRead {
            index,
            dest_byte_offset,
            ..
        } => {
            let index_value = emission_context
                .assigned_target_operations
                .immediate_integer(*index)
                .and_then(|index| u32::try_from(index).ok());
            let destination_byte_offset = u32::try_from(*dest_byte_offset).ok()?;
            if let Some(index) = index_value {
                Some(CheckedInstructionValidationKind::MsrReadImmediateIndex {
                    index,
                    destination_byte_offset,
                })
            } else {
                let index_operand_byte_width =
                    u32::try_from(omega_instruction_selection::runtime_value_operand_width(
                        omega_target::Architecture::X86_64,
                        emission_context.assigned_target_operations,
                        *index,
                    ))
                    .ok()?;
                Some(CheckedInstructionValidationKind::MsrReadRuntimeIndex {
                    index_operand_byte_width,
                    destination_byte_offset,
                })
            }
        }
        SelectedInstructionKind::MsrWrite { index, value } => {
            let value_operand_byte_width =
                u32::try_from(omega_instruction_selection::runtime_value_operand_width(
                    omega_target::Architecture::X86_64,
                    emission_context.assigned_target_operations,
                    *value,
                ))
                .ok()?;
            if let Some(index) = emission_context
                .assigned_target_operations
                .immediate_integer(*index)
                .and_then(|index| u32::try_from(index).ok())
            {
                Some(CheckedInstructionValidationKind::MsrWriteImmediateIndex {
                    index,
                    value_operand_byte_width,
                })
            } else {
                let index_operand_byte_width =
                    u32::try_from(omega_instruction_selection::runtime_value_operand_width(
                        omega_target::Architecture::X86_64,
                        emission_context.assigned_target_operations,
                        *index,
                    ))
                    .ok()?;
                Some(CheckedInstructionValidationKind::MsrWriteRuntimeIndex {
                    index_operand_byte_width,
                    value_operand_byte_width,
                })
            }
        }
        SelectedInstructionKind::ControlRegisterRead {
            register,
            dest_byte_offset,
            ..
        } => {
            let destination_byte_offset = u32::try_from(*dest_byte_offset).ok()?;
            Some(CheckedInstructionValidationKind::ControlRegisterRead {
                register: *register,
                destination_byte_offset,
            })
        }
        SelectedInstructionKind::ControlRegisterWrite { register, source } => {
            let source_operand_byte_width =
                u32::try_from(omega_instruction_selection::runtime_value_operand_width(
                    omega_target::Architecture::X86_64,
                    emission_context.assigned_target_operations,
                    *source,
                ))
                .ok()?;
            Some(CheckedInstructionValidationKind::ControlRegisterWrite {
                register: *register,
                source_operand_byte_width,
            })
        }
        SelectedInstructionKind::FlagsSnapshot {
            dest_byte_offset, ..
        } => {
            let destination_byte_offset = u32::try_from(*dest_byte_offset).ok()?;
            Some(CheckedInstructionValidationKind::FlagsSnapshot {
                destination_byte_offset,
            })
        }
        SelectedInstructionKind::FlagsRestore { source } => {
            let source_operand_byte_width =
                u32::try_from(omega_instruction_selection::runtime_value_operand_width(
                    omega_target::Architecture::X86_64,
                    emission_context.assigned_target_operations,
                    *source,
                ))
                .ok()?;
            Some(CheckedInstructionValidationKind::FlagsRestore {
                source_operand_byte_width,
            })
        }
        _ => None,
    }
}

fn insert_encoded_machine_instruction(
    encoded_bytes: &mut Arena<u8>,
    emission_context: MachineEmissionContext<'_>,
    laid_out_instructions: &[layout::LaidOutMachineInstruction],
    machine_instruction_index: usize,
    kind: &TargetOperationKind,
) -> Result<HandleSpan<u8>, Diagnostic> {
    encoded_bytes.try_insert_many_with(|inserter| {
        if insert_fixed_machine_instruction_bytes(
            inserter,
            emission_context,
            laid_out_instructions,
            machine_instruction_index,
            kind,
        )? {
            return Ok(());
        }

        for byte in encode_machine_instruction_bytes(
            emission_context,
            laid_out_instructions,
            machine_instruction_index,
            kind,
        )? {
            inserter.insert(byte);
        }

        Ok(())
    })
}

fn insert_fixed_machine_instruction_bytes(
    inserter: &mut psi_arena::ArenaSpanInserter<'_, u8>,
    emission_context: MachineEmissionContext<'_>,
    laid_out_instructions: &[layout::LaidOutMachineInstruction],
    machine_instruction_index: usize,
    kind: &TargetOperationKind,
) -> Result<bool, Diagnostic> {
    match kind {
        SelectedInstructionKind::EnterFunction => {
            let (bytes, byte_count) = omega_instruction_selection::encode_function_enter_bytes(
                emission_context.target.architecture,
            )?;
            for byte in bytes.into_iter().take(byte_count) {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::EnterDispatchLoop {
            entry_dispatch_index,
            ..
        } => {
            let bytes = omega_instruction_selection::encode_dispatch_loop_enter_bytes(
                emission_context.target.architecture,
                *entry_dispatch_index,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::EnterDispatchCase { dispatch_index, .. } => {
            let bytes = omega_instruction_selection::encode_dispatch_case_enter_bytes(
                emission_context.target.architecture,
                *dispatch_index,
                branch_distances::byte_distance_to_case_end(
                    laid_out_instructions,
                    machine_instruction_index,
                )?,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: StateGuardLowering::CompareStaticValue,
            operator:
                operator @ (StateGuardOperator::Equal
                | StateGuardOperator::NotEqual
                | StateGuardOperator::Greater
                | StateGuardOperator::GreaterOrEqual
                | StateGuardOperator::Less
                | StateGuardOperator::LessOrEqual
                | StateGuardOperator::GreaterUnsigned
                | StateGuardOperator::GreaterOrEqualUnsigned
                | StateGuardOperator::LessUnsigned
                | StateGuardOperator::LessOrEqualUnsigned),
            byte_offset,
            byte_size,
            expected_value,
            has_storage: true,
            is_float,
            ..
        } => {
            let bytes = omega_instruction_selection::encode_dispatch_guard_compare_static_bytes(
                emission_context.target.architecture,
                *byte_offset,
                *byte_size,
                *expected_value,
                branch_distances::byte_distance_to_next_dispatch_action_end(
                    laid_out_instructions,
                    machine_instruction_index,
                )?,
                *operator,
                *is_float,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        // Forward skip-jump after a matched arm body: a plain unconditional jump to
        // the transition's `BranchArmsEnd` marker, encoded with the same `jmp rel32`
        // as a dispatch-case leave.
        SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: StateGuardLowering::ForwardBranchSkip,
            ..
        } => {
            let bytes = omega_instruction_selection::encode_dispatch_case_leave_bytes(
                emission_context.target.architecture,
                branch_distances::byte_distance_to_branch_arms_end(
                    emission_context,
                    laid_out_instructions,
                    machine_instruction_index,
                )?,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::ComparePlaces {
            left,
            right,
            byte_size,
            operator,
            is_float,
        } => {
            let bytes = omega_instruction_selection::encode_place_compare_bytes(
                emission_context.target.architecture,
                left,
                right,
                *byte_size,
                branch_distances::byte_distance_to_next_runtime_write_end(
                    emission_context,
                    laid_out_instructions,
                    machine_instruction_index,
                )?,
                *operator,
                *is_float,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::ComparePlaceValue {
            place,
            byte_size,
            expected_value,
            operator,
        } => {
            let bytes = omega_instruction_selection::encode_place_value_compare_bytes(
                emission_context.target.architecture,
                place,
                *byte_size,
                *expected_value,
                branch_distances::byte_distance_to_next_runtime_write_end(
                    emission_context,
                    laid_out_instructions,
                    machine_instruction_index,
                )?,
                *operator,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::CompareRuntimeTextStorage {
            buffer,
            source_offset,
            operator,
            ..
        } => {
            let literal_len = emission_context.data.objects.get(*buffer).bytes.len();
            let compare_failure_offset =
                omega_instruction_selection::runtime_text_storage_compare_failure_branch_offset(
                    emission_context.target.architecture,
                    *source_offset,
                    literal_len,
                );
            let delimiter_failure_offset =
                omega_instruction_selection::runtime_text_storage_compare_delimiter_branch_offset(
                    emission_context.target.architecture,
                    *source_offset,
                    literal_len,
                );
            let bytes = omega_instruction_selection::encode_runtime_text_storage_compare_bytes(
                emission_context.target.architecture,
                *source_offset,
                literal_len,
                branch_distances::byte_distance_to_next_guarded_effect_end(
                    emission_context,
                    laid_out_instructions,
                    machine_instruction_index,
                    compare_failure_offset,
                )?,
                branch_distances::byte_distance_to_next_guarded_effect_end(
                    emission_context,
                    laid_out_instructions,
                    machine_instruction_index,
                    delimiter_failure_offset,
                )?,
                *operator == StateGuardOperator::NotEqual,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::SetDispatchState { dispatch_index } => {
            insert_dispatch_state_write_bytes(
                inserter,
                emission_context,
                laid_out_instructions,
                machine_instruction_index,
                *dispatch_index,
            )?;
            Ok(true)
        }
        SelectedInstructionKind::WriteReturnRegisterInteger {
            register,
            byte_size,
            value,
        } => {
            let (bytes, byte_count) =
                omega_instruction_selection::encode_return_register_integer_write_bytes(
                    emission_context.target.architecture,
                    *register,
                    *byte_size,
                    *value,
                )?;
            for byte in bytes.into_iter().take(byte_count) {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::CopyRuntimeStorageToReturnRegister {
            register,
            byte_offset,
            byte_size,
            ..
        } => {
            let bytes =
                omega_instruction_selection::encode_runtime_storage_copy_to_return_register_bytes(
                    emission_context.target.architecture,
                    *register,
                    *byte_offset,
                    *byte_size,
                )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::WriteEntryArgumentRegister {
            register,
            byte_offset,
            byte_size,
        } => {
            let bytes = omega_instruction_selection::encode_entry_argument_register_write_bytes(
                *register,
                *byte_offset,
                *byte_size,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::WriteEntryStackArgument {
            stack_byte_offset,
            byte_offset,
            byte_size,
        } => {
            let bytes = omega_instruction_selection::encode_entry_stack_argument_write_bytes(
                emission_context.target.architecture,
                *stack_byte_offset,
                *byte_offset,
                *byte_size,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::WriteEntryIndirectArgument {
            pointer,
            byte_offset,
            byte_size,
        } => {
            let bytes = omega_instruction_selection::encode_entry_indirect_argument_write_bytes(
                emission_context.target.architecture,
                *pointer,
                *byte_offset,
                *byte_size,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::WriteEntryArgumentsSliceDescriptor {
            descriptor_offset,
            spill_offset,
            byte_length,
        } => {
            let bytes =
                omega_instruction_selection::encode_entry_arguments_slice_descriptor_write_bytes(
                    emission_context.target.architecture,
                    *descriptor_offset,
                    *spill_offset,
                    *byte_length,
                )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::TerminateDispatch => {
            let bytes = omega_instruction_selection::encode_dispatch_state_write_bytes(
                emission_context.target.architecture,
                emission_context.terminal_dispatch_index,
                branch_distances::byte_distance_to_dispatch_loop_leave(
                    emission_context,
                    laid_out_instructions,
                    machine_instruction_index,
                )?,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::LeaveDispatchCase => {
            let bytes = omega_instruction_selection::encode_dispatch_case_leave_bytes(
                emission_context.target.architecture,
                branch_distances::byte_distance_to_dispatch_loop_start(
                    laid_out_instructions,
                    machine_instruction_index,
                )?,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::LeaveFunction => {
            let (bytes, byte_count) = omega_instruction_selection::encode_return_bytes(
                emission_context.target.architecture,
            )?;
            for byte in bytes.into_iter().take(byte_count) {
                inserter.insert(byte);
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn insert_dispatch_state_write_bytes(
    inserter: &mut psi_arena::ArenaSpanInserter<'_, u8>,
    emission_context: MachineEmissionContext<'_>,
    laid_out_instructions: &[layout::LaidOutMachineInstruction],
    machine_instruction_index: usize,
    dispatch_index: u32,
) -> Result<(), Diagnostic> {
    let bytes = omega_instruction_selection::encode_dispatch_state_write_bytes(
        emission_context.target.architecture,
        dispatch_index,
        branch_distances::byte_distance_to_case_leave(
            laid_out_instructions,
            machine_instruction_index,
        )?,
    )?;
    for byte in bytes {
        inserter.insert(byte);
    }
    Ok(())
}
