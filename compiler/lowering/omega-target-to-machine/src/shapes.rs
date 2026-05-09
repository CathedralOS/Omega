mod dispatch;
mod host;
mod runtime_storage;
mod runtime_text;

use crate::TargetToMachineInput;
use omega_target_program::{SelectedInstructionKind, StateGuardLowering, StateGuardOperator};

use omega_machine_program::MachineInstructionKind;

pub(super) fn machine_instruction_shape(
    input: TargetToMachineInput<'_>,
    kind: &SelectedInstructionKind,
) -> (MachineInstructionKind, usize) {
    match kind {
        SelectedInstructionKind::HostOperation {
            capability,
            operation,
            operands,
        } => {
            let operands = input.instructions.operands.span(*operands).unwrap_or(&[]);

            host::host_operation_shape(input, capability, operation, operands)
        }
        SelectedInstructionKind::EnterDispatchLoop {
            entry_dispatch_index,
            ..
        } => dispatch::dispatch_loop_enter_shape(input, *entry_dispatch_index),
        SelectedInstructionKind::EnterDispatchCase { dispatch_index, .. } => {
            dispatch::dispatch_case_enter_shape(input, *dispatch_index)
        }
        SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: StateGuardLowering::CompareStaticValue,
            operator: operator @ (StateGuardOperator::Equal | StateGuardOperator::NotEqual),
            byte_offset,
            byte_size,
            expected_value,
            has_storage: true,
            ..
        } => dispatch::dispatch_guard_compare_static_shape(
            input,
            *operator,
            *byte_offset,
            *byte_size,
            *expected_value,
        ),
        SelectedInstructionKind::CompareRuntimeTextLiteral { literal, .. } => {
            runtime_text::runtime_text_literal_compare_shape(input, literal)
        }
        SelectedInstructionKind::CompareRuntimeTextStorage {
            source_offset,
            operator,
            ..
        } => runtime_text::runtime_text_storage_compare_shape(input, *source_offset, *operator),
        SelectedInstructionKind::CompareRuntimeStorage {
            left_offset,
            right_offset,
            byte_size,
            operator,
            ..
        } => runtime_storage::runtime_storage_compare_shape(
            input,
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
        } => runtime_storage::runtime_storage_value_compare_shape(
            input,
            *byte_offset,
            *byte_size,
            *expected_value,
            *operator,
        ),
        SelectedInstructionKind::WriteRuntimeTextLiteral { literal, .. } => {
            runtime_text::runtime_text_literal_write_shape(input, literal)
        }
        SelectedInstructionKind::WriteRuntimeTextLiteralSegment {
            byte_offset,
            literal,
            ..
        } => runtime_text::runtime_text_literal_segment_write_shape(input, *byte_offset, literal),
        SelectedInstructionKind::AppendRuntimeTextStoredSuffix {
            buffer_offset,
            source_offset,
            target_offset,
            length_delta,
            ..
        } => runtime_text::runtime_text_stored_suffix_append_shape(
            input,
            *buffer_offset,
            *source_offset,
            *target_offset,
            *length_delta,
        ),
        SelectedInstructionKind::MaterializeRuntimeTextBuffer { target_offset, .. } => {
            runtime_text::runtime_text_buffer_materialize_shape(input, *target_offset)
        }
        SelectedInstructionKind::AppendRuntimeTextStoredPlace {
            source_offset,
            target_offset,
            ..
        } => runtime_text::runtime_text_stored_place_append_shape(
            input,
            *source_offset,
            *target_offset,
        ),
        SelectedInstructionKind::AppendRuntimeTextLiteral {
            target_offset,
            literal,
            ..
        } => runtime_text::runtime_text_literal_append_shape(input, *target_offset, literal),
        SelectedInstructionKind::WriteRuntimeMachineInteger {
            byte_offset,
            byte_size,
            value,
        } => runtime_storage::runtime_machine_integer_write_shape(
            input,
            *byte_offset,
            *byte_size,
            *value,
        ),
        SelectedInstructionKind::WriteRuntimeMachineString {
            byte_offset,
            byte_length,
            ..
        } => runtime_storage::runtime_machine_string_write_shape(input, *byte_offset, *byte_length),
        SelectedInstructionKind::ReadRuntimeTextLine {
            target_offset,
            byte_capacity,
            source,
            ..
        } => runtime_text::runtime_text_line_read_shape(
            input,
            *target_offset,
            *byte_capacity,
            source,
        ),
        SelectedInstructionKind::CopyRuntimeStorage {
            source_offset,
            target_offset,
            byte_count,
            ..
        } => runtime_storage::runtime_storage_copy_shape(
            input,
            *source_offset,
            *target_offset,
            *byte_count,
        ),
        SelectedInstructionKind::SetDispatchState { dispatch_index } => {
            dispatch::dispatch_state_write_shape(input, *dispatch_index)
        }
        SelectedInstructionKind::TerminateDispatch => dispatch::dispatch_terminate_shape(input),
        SelectedInstructionKind::LeaveDispatchCase => dispatch::dispatch_case_leave_shape(input),
        SelectedInstructionKind::LeaveFunction => dispatch::return_shape(input),
        SelectedInstructionKind::EnterFunction
        | SelectedInstructionKind::EvaluateDispatchGuard { .. }
        | SelectedInstructionKind::LeaveDispatchLoop
        | SelectedInstructionKind::BeginPlatformCall => (MachineInstructionKind::NoBytes, 0),
    }
}
