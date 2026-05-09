mod dispatch;
mod host;
mod runtime_storage;
mod runtime_text;

use crate::plan::NativePlan;
use crate::state_guards::{StateGuardLowering, StateGuardOperator};
use omega_core::diagnostics::Diagnostic;
use omega_instruction_selection as architecture;
use omega_target_program::SelectedInstructionKind;

use omega_machine_program::MachineInstruction;

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

            host::encode_host_operation(native_plan, capability, operation, operands)
        }
        SelectedInstructionKind::EnterDispatchLoop {
            entry_dispatch_index,
            ..
        } => dispatch::encode_dispatch_loop_enter(native_plan, *entry_dispatch_index),
        SelectedInstructionKind::EnterDispatchCase { dispatch_index, .. } => {
            dispatch::encode_dispatch_case_enter(
                native_plan,
                machine_instructions,
                machine_instruction_index,
                *dispatch_index,
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
        } => dispatch::encode_dispatch_guard_compare_static(
            native_plan,
            machine_instructions,
            machine_instruction_index,
            *byte_offset,
            *byte_size,
            *expected_value,
            *operator,
        ),
        SelectedInstructionKind::CompareRuntimeTextLiteral { literal, .. } => {
            runtime_text::encode_runtime_text_literal_compare(
                native_plan,
                machine_instructions,
                machine_instruction_index,
                literal,
            )
        }
        SelectedInstructionKind::CompareRuntimeTextStorage {
            source_offset,
            operator,
            ..
        } => runtime_text::encode_runtime_text_storage_compare(
            native_plan,
            machine_instructions,
            machine_instruction_index,
            *source_offset,
            *operator,
        ),
        SelectedInstructionKind::CompareRuntimeStorage {
            left_offset,
            right_offset,
            byte_size,
            operator,
            ..
        } => runtime_storage::encode_runtime_storage_compare(
            native_plan,
            machine_instructions,
            machine_instruction_index,
            *left_offset,
            *right_offset,
            *byte_size,
            *operator,
        ),
        SelectedInstructionKind::CompareRuntimeStorageValue {
            byte_offset,
            byte_size,
            expected_value,
            operator,
            ..
        } => runtime_storage::encode_runtime_storage_value_compare(
            native_plan,
            machine_instructions,
            machine_instruction_index,
            *byte_offset,
            *byte_size,
            *expected_value,
            *operator,
        ),
        SelectedInstructionKind::WriteRuntimeTextLiteral { literal, .. } => {
            runtime_text::encode_runtime_text_literal_write(native_plan, literal)
        }
        SelectedInstructionKind::WriteRuntimeTextLiteralSegment {
            byte_offset,
            literal,
            ..
        } => runtime_text::encode_runtime_text_literal_segment_write(
            native_plan,
            *byte_offset,
            literal,
        ),
        SelectedInstructionKind::AppendRuntimeTextStoredSuffix {
            buffer_offset,
            source_offset,
            target_offset,
            length_delta,
            ..
        } => runtime_text::encode_runtime_text_stored_suffix_append(
            native_plan,
            *buffer_offset,
            *source_offset,
            *target_offset,
            *length_delta,
        ),
        SelectedInstructionKind::AppendRuntimeTextStoredPlace {
            source_offset,
            target_offset,
            ..
        } => runtime_text::encode_runtime_text_stored_place_append(
            native_plan,
            *source_offset,
            *target_offset,
        ),
        SelectedInstructionKind::AppendRuntimeTextLiteral {
            target_offset,
            literal,
            ..
        } => runtime_text::encode_runtime_text_literal_append(native_plan, *target_offset, literal),
        SelectedInstructionKind::MaterializeRuntimeTextBuffer { target_offset, .. } => {
            runtime_text::encode_runtime_text_buffer_materialize(native_plan, *target_offset)
        }
        SelectedInstructionKind::WriteRuntimeMachineInteger {
            byte_offset,
            byte_size,
            value,
        } => runtime_storage::encode_runtime_machine_integer_write(
            native_plan,
            *byte_offset,
            *byte_size,
            *value,
        ),
        SelectedInstructionKind::WriteRuntimeMachineString {
            byte_offset,
            byte_length,
            ..
        } => runtime_storage::encode_runtime_machine_string_write(
            native_plan,
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
        } => runtime_text::encode_runtime_text_line_read(
            native_plan,
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
        } => runtime_storage::encode_runtime_storage_copy(
            native_plan,
            *source_offset,
            *target_offset,
            *byte_count,
        ),
        SelectedInstructionKind::SetDispatchState { dispatch_index } => {
            dispatch::encode_dispatch_state_write(
                native_plan,
                machine_instructions,
                machine_instruction_index,
                *dispatch_index,
            )
        }
        SelectedInstructionKind::TerminateDispatch => dispatch::encode_dispatch_terminal_write(
            native_plan,
            machine_instructions,
            machine_instruction_index,
        ),
        SelectedInstructionKind::LeaveDispatchCase => dispatch::encode_dispatch_case_leave(
            native_plan,
            machine_instructions,
            machine_instruction_index,
        ),
        SelectedInstructionKind::LeaveFunction => {
            architecture::encode_return(native_plan.target.architecture)
        }
        SelectedInstructionKind::EnterFunction
        | SelectedInstructionKind::EvaluateDispatchGuard { .. }
        | SelectedInstructionKind::LeaveDispatchLoop
        | SelectedInstructionKind::BeginPlatformCall => Ok(Vec::new()),
    }
}
