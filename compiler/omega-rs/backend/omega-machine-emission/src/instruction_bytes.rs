use crate::MachineEmissionContext;
use crate::branch_distances;
use crate::encoding::encode_machine_instruction_bytes;
use crate::layout::{self, layout_machine_instructions};
use omega_assigned_target_operations::{
    SelectedInstructionKind, StateGuardLowering, StateGuardOperator, TargetOperationKind,
};
use omega_core::arena::{Arena, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_machine_bytes::{EncodedMachineCode, EncodedMachineInstruction};
use omega_machine_instructions::{MachineInstruction, MachineInstructionPlan};

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
        if laid_out_instruction.byte_width == 0 {
            encoded_code.instructions.insert(EncodedMachineInstruction {
                selected_instruction_index: machine_instruction.selected_instruction_index,
                bytes: HandleSpan::empty(),
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
                SelectedInstructionKind::WriteRuntimeStorageBinary { left, right, .. }
                | SelectedInstructionKind::WriteRuntimePointeeBinary { left, right, .. }
                | SelectedInstructionKind::WriteRuntimeFrameIndexedBinary { left, right, .. } => {
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
        encoded_code.instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: machine_instruction.selected_instruction_index,
            bytes: byte_span,
        });
    }

    Ok(())
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
    inserter: &mut omega_core::arena::ArenaSpanInserter<'_, u8>,
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
        SelectedInstructionKind::CompareRuntimeStorage {
            left_offset,
            right_offset,
            byte_size,
            operator,
            is_float,
            ..
        } => {
            let bytes = omega_instruction_selection::encode_runtime_storage_compare_bytes(
                emission_context.target.architecture,
                *left_offset,
                *right_offset,
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
        SelectedInstructionKind::CompareRuntimeStorageValue {
            byte_offset,
            byte_size,
            expected_value,
            operator,
            ..
        } => {
            let bytes = omega_instruction_selection::encode_runtime_storage_value_compare_bytes(
                emission_context.target.architecture,
                *byte_offset,
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
        SelectedInstructionKind::WriteReturnRegisterInteger { byte_size, value } => {
            let (bytes, byte_count) =
                omega_instruction_selection::encode_return_register_integer_write_bytes(
                    emission_context.target.architecture,
                    *byte_size,
                    *value,
                )?;
            for byte in bytes.into_iter().take(byte_count) {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::CopyRuntimeStorageToReturnRegister {
            byte_offset,
            byte_size,
            ..
        } => {
            let bytes =
                omega_instruction_selection::encode_runtime_storage_copy_to_return_register_bytes(
                    emission_context.target.architecture,
                    *byte_offset,
                    *byte_size,
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
    inserter: &mut omega_core::arena::ArenaSpanInserter<'_, u8>,
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
