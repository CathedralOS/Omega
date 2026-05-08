use crate::abi::HostBindingMechanism;
use crate::architecture;
use crate::instructions::SelectedInstructionKind;
use crate::plan::NativePlan;
use crate::state_guards::{StateGuardLowering, StateGuardOperator};
use omega_core::diagnostics::Diagnostic;

use super::branch_distances::{
    byte_distance_to_case_end, byte_distance_to_case_leave, byte_distance_to_dispatch_loop_start,
    byte_distance_to_next_runtime_write_end,
    byte_distance_to_next_runtime_write_end_from_branch_offset,
    byte_distance_to_next_state_write_end, byte_distances_to_next_runtime_machine_write_end,
};
use super::host_bindings::host_binding_mechanism;
use super::model::MachineInstruction;
use super::widths::runtime_text_storage_compare_width;

pub(super) fn encode_machine_instruction(
    native_plan: &NativePlan,
    machine_instructions: &[MachineInstruction],
    machine_instruction_index: usize,
    kind: &SelectedInstructionKind,
) -> Result<Vec<u8>, Diagnostic> {
    match kind {
        SelectedInstructionKind::HostOperation {
            capability,
            operation,
            operands,
        } => {
            let Some(operands) = native_plan.instructions.operands.span(*operands) else {
                return Ok(Vec::new());
            };

            match host_binding_mechanism(native_plan, capability, operation) {
                Some(HostBindingMechanism::Syscall {
                    number,
                    number_register,
                    supervisor_call,
                    ..
                }) => architecture::encode_syscall_sequence(
                    native_plan.target.architecture,
                    operands,
                    *number,
                    *number_register,
                    *supervisor_call,
                ),
                _ => architecture::encode_host_call_sequence(
                    native_plan.target.architecture,
                    operands,
                ),
            }
        }
        SelectedInstructionKind::EnterDispatchLoop {
            entry_dispatch_index,
            ..
        } => architecture::encode_dispatch_loop_enter(
            native_plan.target.architecture,
            *entry_dispatch_index,
        ),
        SelectedInstructionKind::EnterDispatchCase { dispatch_index, .. } => {
            architecture::encode_dispatch_case_enter(
                native_plan.target.architecture,
                *dispatch_index,
                byte_distance_to_case_end(machine_instructions, machine_instruction_index)?,
            )
        }
        SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: StateGuardLowering::CompareStaticValue,
            operator: operator @ (StateGuardOperator::Equal | StateGuardOperator::NotEqual),
            byte_offset,
            byte_size,
            expected_value,
            has_storage: true,
            ..
        } => architecture::encode_dispatch_guard_compare_static(
            native_plan.target.architecture,
            *byte_offset,
            *byte_size,
            *expected_value,
            byte_distance_to_next_state_write_end(machine_instructions, machine_instruction_index)?,
            *operator == StateGuardOperator::NotEqual,
        ),
        SelectedInstructionKind::CompareRuntimeTextLiteral { literal, .. } => {
            architecture::encode_runtime_text_literal_compare(
                native_plan.target.architecture,
                literal,
                byte_distances_to_next_runtime_machine_write_end(
                    native_plan,
                    machine_instructions,
                    machine_instruction_index,
                    literal,
                )?,
                byte_distance_to_next_runtime_write_end(
                    native_plan,
                    machine_instructions,
                    machine_instruction_index,
                )?,
            )
        }
        SelectedInstructionKind::CompareRuntimeTextStorage {
            source_offset,
            operator,
            ..
        } => architecture::encode_runtime_text_storage_compare(
            native_plan.target.architecture,
            *source_offset,
            byte_distance_to_next_runtime_write_end_from_branch_offset(
                native_plan,
                machine_instructions,
                machine_instruction_index,
                40,
            )?,
            byte_distance_to_next_runtime_write_end_from_branch_offset(
                native_plan,
                machine_instructions,
                machine_instruction_index,
                runtime_text_storage_compare_width(native_plan.target.architecture)
                    .saturating_sub(4),
            )?,
            *operator == StateGuardOperator::NotEqual,
        ),
        SelectedInstructionKind::CompareRuntimeStorage {
            left_offset,
            right_offset,
            byte_size,
            operator,
            ..
        } => architecture::encode_runtime_storage_compare(
            native_plan.target.architecture,
            *left_offset,
            *right_offset,
            *byte_size,
            byte_distance_to_next_runtime_write_end(
                native_plan,
                machine_instructions,
                machine_instruction_index,
            )?,
            *operator == StateGuardOperator::NotEqual,
        ),
        SelectedInstructionKind::CompareRuntimeStorageValue {
            byte_offset,
            byte_size,
            expected_value,
            operator,
            ..
        } => architecture::encode_runtime_storage_value_compare(
            native_plan.target.architecture,
            *byte_offset,
            *byte_size,
            *expected_value,
            byte_distance_to_next_runtime_write_end(
                native_plan,
                machine_instructions,
                machine_instruction_index,
            )?,
            *operator == StateGuardOperator::NotEqual,
        ),
        SelectedInstructionKind::WriteRuntimeTextLiteral { literal, .. } => {
            architecture::encode_runtime_text_literal_write(
                native_plan.target.architecture,
                literal,
            )
        }
        SelectedInstructionKind::WriteRuntimeTextLiteralSegment {
            byte_offset,
            literal,
            ..
        } => architecture::encode_runtime_text_literal_segment_write(
            native_plan.target.architecture,
            *byte_offset,
            literal,
        ),
        SelectedInstructionKind::AppendRuntimeTextStoredSuffix {
            buffer_offset,
            source_offset,
            target_offset,
            length_delta,
            ..
        } => architecture::encode_runtime_text_stored_suffix_append(
            native_plan.target.architecture,
            *buffer_offset,
            *source_offset,
            *target_offset,
            *length_delta,
        ),
        SelectedInstructionKind::AppendRuntimeTextStoredPlace {
            source_offset,
            target_offset,
            ..
        } => architecture::encode_runtime_text_stored_place_append(
            native_plan.target.architecture,
            0,
            *source_offset,
            *target_offset,
        ),
        SelectedInstructionKind::AppendRuntimeTextLiteral {
            target_offset,
            literal,
            ..
        } => architecture::encode_runtime_text_literal_append(
            native_plan.target.architecture,
            0,
            *target_offset,
            literal,
        ),
        SelectedInstructionKind::MaterializeRuntimeTextBuffer { target_offset, .. } => {
            architecture::encode_runtime_text_buffer_materialize(
                native_plan.target.architecture,
                *target_offset,
            )
        }
        SelectedInstructionKind::WriteRuntimeMachineInteger {
            byte_offset,
            byte_size,
            value,
        } => architecture::encode_runtime_machine_integer_write(
            native_plan.target.architecture,
            *byte_offset,
            *byte_size,
            *value,
        ),
        SelectedInstructionKind::WriteRuntimeMachineString {
            byte_offset,
            byte_length,
            ..
        } => architecture::encode_runtime_machine_string_write(
            native_plan.target.architecture,
            *byte_offset,
            *byte_length,
        ),
        SelectedInstructionKind::ReadRuntimeTextLine {
            target_offset,
            byte_capacity,
            syscall_number,
            syscall_number_register,
            supervisor_call,
            ..
        } => architecture::encode_runtime_text_line_read(
            native_plan.target.architecture,
            *target_offset,
            *byte_capacity,
            *syscall_number,
            *syscall_number_register,
            *supervisor_call,
        ),
        SelectedInstructionKind::CopyRuntimeStorage {
            source_offset,
            target_offset,
            byte_count,
            ..
        } => architecture::encode_runtime_storage_copy(
            native_plan.target.architecture,
            *source_offset,
            *target_offset,
            *byte_count,
        ),
        SelectedInstructionKind::SetDispatchState { dispatch_index } => {
            architecture::encode_dispatch_state_write(
                native_plan.target.architecture,
                *dispatch_index,
                byte_distance_to_case_leave(machine_instructions, machine_instruction_index)?,
            )
        }
        SelectedInstructionKind::TerminateDispatch => architecture::encode_dispatch_state_write(
            native_plan.target.architecture,
            native_plan.runtime_dispatch_loop.terminal_dispatch_index,
            byte_distance_to_case_leave(machine_instructions, machine_instruction_index)?,
        ),
        SelectedInstructionKind::LeaveDispatchCase => architecture::encode_dispatch_case_leave(
            native_plan.target.architecture,
            byte_distance_to_dispatch_loop_start(machine_instructions, machine_instruction_index)?,
        ),
        SelectedInstructionKind::LeaveFunction => {
            architecture::encode_return(native_plan.target.architecture)
        }
        SelectedInstructionKind::EnterFunction
        | SelectedInstructionKind::EvaluateDispatchGuard { .. }
        | SelectedInstructionKind::LeaveDispatchLoop
        | SelectedInstructionKind::BeginPlatformCall { .. } => Ok(Vec::new()),
    }
}
