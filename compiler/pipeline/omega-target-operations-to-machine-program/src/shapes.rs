mod dispatch;
mod host;
mod runtime_storage;
mod runtime_text;

use omega_core::diagnostics::Diagnostic;
use omega_machine_program::MachineInstructionKind;
use omega_target_operations::{SelectedInstructionKind, StateGuardLowering, StateGuardOperator};

pub(super) fn lower_machine_instruction_kind(
    kind: &SelectedInstructionKind,
) -> Result<MachineInstructionKind, Diagnostic> {
    Ok(match kind {
        SelectedInstructionKind::HostOperation { operation_key, .. } => {
            host::host_operation_kind(*operation_key)
        }
        SelectedInstructionKind::EnterDispatchLoop {
            entry_dispatch_index,
            ..
        } => dispatch::dispatch_loop_enter_kind(*entry_dispatch_index),
        SelectedInstructionKind::EnterDispatchCase { dispatch_index, .. } => {
            dispatch::dispatch_case_enter_kind(*dispatch_index)
        }
        SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: StateGuardLowering::CompareStaticValue,
            operator: operator @ (StateGuardOperator::Equal | StateGuardOperator::NotEqual),
            byte_offset,
            byte_size,
            expected_value,
            has_storage: true,
            ..
        } => dispatch::dispatch_guard_compare_static_kind(
            *operator,
            *byte_offset,
            *byte_size,
            *expected_value,
        ),
        SelectedInstructionKind::CompareRuntimeTextLiteral { literal, .. } => {
            runtime_text::runtime_text_literal_compare_kind(literal)
        }
        SelectedInstructionKind::CompareRuntimeTextStorage {
            source_offset,
            operator,
            ..
        } => runtime_text::runtime_text_storage_compare_kind(*source_offset, *operator),
        SelectedInstructionKind::CompareRuntimeStorage {
            left_offset,
            right_offset,
            byte_size,
            operator,
            ..
        } => runtime_storage::runtime_storage_compare_kind(
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
        } => runtime_storage::runtime_storage_value_compare_kind(
            *byte_offset,
            *byte_size,
            *expected_value,
            *operator,
        ),
        SelectedInstructionKind::WriteRuntimeTextLiteral { literal, .. } => {
            runtime_text::runtime_text_literal_write_kind(literal)
        }
        SelectedInstructionKind::WriteRuntimeTextLiteralSegment {
            byte_offset,
            literal,
            ..
        } => runtime_text::runtime_text_literal_segment_write_kind(*byte_offset, literal),
        SelectedInstructionKind::AppendRuntimeTextStoredSuffix {
            buffer_offset,
            source_offset,
            target_offset,
            length_delta,
            ..
        } => runtime_text::runtime_text_stored_suffix_append_kind(
            *buffer_offset,
            *source_offset,
            *target_offset,
            *length_delta,
        ),
        SelectedInstructionKind::MaterializeRuntimeTextBuffer { target_offset, .. } => {
            runtime_text::runtime_text_buffer_materialize_kind(*target_offset)
        }
        SelectedInstructionKind::AppendRuntimeTextStoredPlace {
            source_offset,
            target_offset,
            ..
        } => runtime_text::runtime_text_stored_place_append_kind(*source_offset, *target_offset),
        SelectedInstructionKind::AppendRuntimeTextLiteral {
            target_offset,
            literal,
            ..
        } => runtime_text::runtime_text_literal_append_kind(*target_offset, literal),
        SelectedInstructionKind::WriteRuntimeMachineInteger {
            byte_offset,
            byte_size,
            value,
        } => runtime_storage::runtime_machine_integer_write_kind(*byte_offset, *byte_size, *value),
        SelectedInstructionKind::WriteRuntimeStorageInteger {
            byte_offset,
            byte_size,
            value,
            ..
        } => runtime_storage::runtime_storage_integer_write_kind(*byte_offset, *byte_size, *value),
        SelectedInstructionKind::WriteRuntimeStorageBinary {
            target_offset,
            byte_size,
            left,
            operator,
            right,
            ..
        } => runtime_storage::runtime_storage_binary_write_kind(
            *target_offset,
            *byte_size,
            *left,
            *operator,
            *right,
        ),
        SelectedInstructionKind::WriteRuntimeFrameIndexedInteger {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            value,
        } => runtime_storage::runtime_frame_indexed_integer_write_kind(
            *descriptor_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
            *byte_size,
            *value,
        ),
        SelectedInstructionKind::WriteRuntimeFrameIndexedBinary {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
        } => runtime_storage::runtime_frame_indexed_binary_write_kind(
            *descriptor_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
            *byte_size,
            *left,
            *operator,
            *right,
        ),
        SelectedInstructionKind::WriteRuntimeMachineString {
            byte_offset,
            byte_length,
            ..
        } => runtime_storage::runtime_machine_string_write_kind(*byte_offset, *byte_length),
        SelectedInstructionKind::ReadRuntimeTextLine {
            target_offset,
            byte_capacity,
            source,
            ..
        } => runtime_text::runtime_text_line_read_kind(*target_offset, *byte_capacity, source),
        SelectedInstructionKind::CopyRuntimeStorage {
            source_offset,
            target_offset,
            byte_count,
            ..
        } => runtime_storage::runtime_storage_copy_kind(*source_offset, *target_offset, *byte_count),
        SelectedInstructionKind::CopyRuntimeStorageToRuntimeFrameIndexed {
            source_offset,
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_count,
            ..
        } => runtime_storage::runtime_storage_copy_to_runtime_frame_indexed_kind(
            *source_offset,
            *descriptor_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
            *byte_count,
        ),
        SelectedInstructionKind::SetDispatchState { dispatch_index } => {
            dispatch::dispatch_state_write_kind(*dispatch_index)
        }
        SelectedInstructionKind::TerminateDispatch => dispatch::dispatch_terminate_kind(),
        SelectedInstructionKind::LeaveDispatchCase => dispatch::dispatch_case_leave_kind(),
        SelectedInstructionKind::LeaveFunction => dispatch::return_kind(),
        SelectedInstructionKind::EnterFunction
        | SelectedInstructionKind::EvaluateDispatchGuard { .. }
        | SelectedInstructionKind::LeaveDispatchLoop
        | SelectedInstructionKind::BeginPlatformCall => MachineInstructionKind::NoOp,
    })
}
