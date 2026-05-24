mod dispatch;
mod host;
mod runtime_storage;
mod runtime_text;

use omega_assigned_target_operations::{
    AssignedValueHomeKind, SelectedInstructionKind, StateGuardLowering, StateGuardOperator,
};
use omega_core::diagnostics::Diagnostic;
use omega_core::arena::Handle;
use omega_machine_instructions::MachineInstructionKind;

pub(super) fn lower_machine_instruction_kind(
    assigned_target_operations: &omega_assigned_target_operations::AssignedTargetOperationPlan,
    selected_instruction_handle: Handle<omega_assigned_target_operations::SelectedInstruction>,
    kind: &SelectedInstructionKind,
) -> Result<MachineInstructionKind, Diagnostic> {
    ensure_runtime_value_homes(assigned_target_operations, selected_instruction_handle, kind)?;
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
            operator:
                operator @ (StateGuardOperator::Equal
                | StateGuardOperator::NotEqual
                | StateGuardOperator::Greater
                | StateGuardOperator::GreaterOrEqual
                | StateGuardOperator::Less
                | StateGuardOperator::LessOrEqual),
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
        SelectedInstructionKind::EvaluateDispatchGuard { .. } => MachineInstructionKind::NoOp,
        SelectedInstructionKind::CompareRuntimeTextLiteral { .. } => {
            runtime_text::runtime_text_literal_compare_kind()
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
        SelectedInstructionKind::CompareRuntimeValues { .. } => MachineInstructionKind::NoOp,
        SelectedInstructionKind::WriteRuntimeTextLiteral { .. } => {
            runtime_text::runtime_text_literal_write_kind()
        }
        SelectedInstructionKind::WriteRuntimeTextLiteralSegment { byte_offset, .. } => {
            runtime_text::runtime_text_literal_segment_write_kind(*byte_offset)
        }
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
        SelectedInstructionKind::MaterializeRuntimeTextBufferToRuntimePointee {
            pointer_byte_offset,
            field_byte_offset,
            ..
        } => runtime_text::runtime_text_buffer_materialize_to_runtime_pointee_kind(
            *pointer_byte_offset,
            *field_byte_offset,
        ),
        SelectedInstructionKind::MaterializeRuntimeTextBufferToRuntimeFrameIndexed {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            ..
        } => runtime_text::runtime_text_buffer_materialize_to_runtime_frame_indexed_kind(
            *descriptor_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
        ),
        SelectedInstructionKind::AppendRuntimeTextStoredPlace {
            source_offset,
            target_offset,
            ..
        } => runtime_text::runtime_text_stored_place_append_kind(*source_offset, *target_offset),
        SelectedInstructionKind::AppendRuntimeTextStoredPlaceToRuntimePointee {
            source_offset,
            pointer_byte_offset,
            field_byte_offset,
            ..
        } => runtime_text::runtime_text_stored_place_append_to_runtime_pointee_kind(
            *source_offset,
            *pointer_byte_offset,
            *field_byte_offset,
        ),
        SelectedInstructionKind::AppendRuntimeTextStoredPlaceToRuntimeFrameIndexed {
            source_offset,
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            ..
        } => runtime_text::runtime_text_stored_place_append_to_runtime_frame_indexed_kind(
            *source_offset,
            *descriptor_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
        ),
        SelectedInstructionKind::AppendRuntimeTextLiteral { target_offset, .. } => {
            runtime_text::runtime_text_literal_append_kind(*target_offset)
        }
        SelectedInstructionKind::AppendRuntimeTextLiteralToRuntimePointee {
            pointer_byte_offset,
            field_byte_offset,
            ..
        } => runtime_text::runtime_text_literal_append_to_runtime_pointee_kind(
            *pointer_byte_offset,
            *field_byte_offset,
        ),
        SelectedInstructionKind::AppendRuntimeTextLiteralToRuntimeFrameIndexed {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            ..
        } => runtime_text::runtime_text_literal_append_to_runtime_frame_indexed_kind(
            *descriptor_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
        ),
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
        SelectedInstructionKind::WriteRuntimePointeeInteger {
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
            value,
        } => runtime_storage::runtime_pointee_integer_write_kind(
            *pointer_byte_offset,
            *field_byte_offset,
            *byte_size,
            *value,
        ),
        SelectedInstructionKind::WriteRuntimeStorageBinary {
            target_offset,
            byte_size,
            left: _,
            operator,
            right: _,
            ..
        } => runtime_storage::runtime_storage_binary_write_kind(
            *target_offset,
            *byte_size,
            *operator,
        ),
        SelectedInstructionKind::WriteRuntimePointeeBinary {
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
            left: _,
            operator,
            right: _,
        } => runtime_storage::runtime_pointee_binary_write_kind(
            *pointer_byte_offset,
            *field_byte_offset,
            *byte_size,
            *operator,
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
            left: _,
            operator,
            right: _,
        } => runtime_storage::runtime_frame_indexed_binary_write_kind(
            *descriptor_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
            *byte_size,
            *operator,
        ),
        SelectedInstructionKind::WriteRuntimeMachineString {
            byte_offset,
            byte_length,
            ..
        } => runtime_storage::runtime_machine_string_write_kind(*byte_offset, *byte_length),
        SelectedInstructionKind::WriteRuntimePointeeString {
            pointer_byte_offset,
            field_byte_offset,
            byte_length,
            ..
        } => runtime_storage::runtime_pointee_string_write_kind(
            *pointer_byte_offset,
            *field_byte_offset,
            *byte_length,
        ),
        SelectedInstructionKind::WriteRuntimeFrameIndexedString {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_length,
            ..
        } => runtime_storage::runtime_frame_indexed_string_write_kind(
            *descriptor_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
            *byte_length,
        ),
        SelectedInstructionKind::WriteRuntimeStorageAddressToRuntimeFrame {
            source_offset,
            target_offset,
            ..
        } => runtime_storage::runtime_storage_address_to_runtime_frame_write_kind(
            *source_offset,
            *target_offset,
        ),
        SelectedInstructionKind::WriteRuntimePointeeAddressToRuntimeFrame {
            pointer_byte_offset,
            field_byte_offset,
            target_offset,
            ..
        } => runtime_storage::runtime_pointee_address_to_runtime_frame_write_kind(
            *pointer_byte_offset,
            *field_byte_offset,
            *target_offset,
        ),
        SelectedInstructionKind::ReadRuntimeTextLine {
            target_offset,
            byte_capacity,
            ..
        } => runtime_text::runtime_text_line_read_kind(*target_offset, *byte_capacity),
        SelectedInstructionKind::CopyRuntimeStorage {
            source_offset,
            target_offset,
            byte_count,
            ..
        } => runtime_storage::runtime_storage_copy_kind(
            *source_offset,
            *target_offset,
            *byte_count,
        ),
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
        SelectedInstructionKind::CopyRuntimeFrameIndexedToRuntimeFrame {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
            ..
        } => runtime_storage::runtime_storage_copy_from_runtime_frame_indexed_kind(
            *descriptor_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
            *target_offset,
            *byte_count,
        ),
        SelectedInstructionKind::CopyRuntimeFrameFixedIndexedToRuntimeFrame {
            descriptor_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
            ..
        } => runtime_storage::runtime_storage_copy_from_runtime_frame_fixed_indexed_kind(
            *descriptor_offset,
            *element_index,
            *element_byte_size,
            *field_byte_offset,
            *target_offset,
            *byte_count,
        ),
        SelectedInstructionKind::CopyRuntimeStorageToRuntimePointee {
            source_offset,
            pointer_byte_offset,
            field_byte_offset,
            byte_count,
            ..
        } => runtime_storage::runtime_storage_copy_to_runtime_pointee_kind(
            *source_offset,
            *pointer_byte_offset,
            *field_byte_offset,
            *byte_count,
        ),
        SelectedInstructionKind::SetDispatchState { dispatch_index } => {
            dispatch::dispatch_state_write_kind(*dispatch_index)
        }
        SelectedInstructionKind::WriteReturnRegisterInteger { .. } => {
            MachineInstructionKind::ReturnRegisterIntegerWrite
        }
        SelectedInstructionKind::TerminateDispatch => dispatch::dispatch_terminate_kind(),
        SelectedInstructionKind::LeaveDispatchCase => dispatch::dispatch_case_leave_kind(),
        SelectedInstructionKind::LeaveFunction => dispatch::return_kind(),
        SelectedInstructionKind::EnterFunction
        | SelectedInstructionKind::LeaveDispatchLoop
        | SelectedInstructionKind::BeginPlatformCall => MachineInstructionKind::NoOp,
    })
}

fn ensure_runtime_value_homes(
    assigned_target_operations: &omega_assigned_target_operations::AssignedTargetOperationPlan,
    selected_instruction_handle: Handle<omega_assigned_target_operations::SelectedInstruction>,
    kind: &SelectedInstructionKind,
) -> Result<(), Diagnostic> {
    let selected_instruction = assigned_target_operations.instructions.get(selected_instruction_handle);
    for handle in [first_runtime_value_handle(kind), second_runtime_value_handle(kind)]
        .into_iter()
        .flatten()
    {
        let home_handle = runtime_value_home_handle(handle);
        if !assigned_target_operations.runtime_value_homes.is_valid(home_handle) {
            return Err(Diagnostic::error(format!(
                "missing assigned value home for {:?} in {:?} statement {}",
                handle,
                selected_instruction.source_key,
                selected_instruction.source_statement
            )));
        }
        let home = assigned_target_operations.runtime_value_homes.get(home_handle);

        if matches!(
            assigned_target_operations.runtime_value_operands.get(handle),
            omega_assigned_target_operations::RuntimeValueOperand::Binary { .. }
        ) && !matches!(home.kind, AssignedValueHomeKind::ScratchRegister { .. })
        {
            return Err(Diagnostic::error(format!(
                "binary runtime value {:?} in {:?} statement {} must lower through a scratch register home",
                handle,
                selected_instruction.source_key,
                selected_instruction.source_statement
            )));
        }
    }

    Ok(())
}

fn runtime_value_home_handle(
    handle: omega_assigned_target_operations::RuntimeValueOperandHandle,
) -> omega_assigned_target_operations::AssignedValueHomeHandle {
    omega_core::arena::Handle::from_arena_index(handle.arena_index())
}

fn first_runtime_value_handle(
    kind: &SelectedInstructionKind,
) -> Option<omega_assigned_target_operations::RuntimeValueOperandHandle> {
    match kind {
        SelectedInstructionKind::CompareRuntimeValues { left, .. }
        | SelectedInstructionKind::WriteRuntimeStorageBinary { left, .. }
        | SelectedInstructionKind::WriteRuntimePointeeBinary { left, .. }
        | SelectedInstructionKind::WriteRuntimeFrameIndexedBinary { left, .. } => Some(*left),
        _ => None,
    }
}

fn second_runtime_value_handle(
    kind: &SelectedInstructionKind,
) -> Option<omega_assigned_target_operations::RuntimeValueOperandHandle> {
    match kind {
        SelectedInstructionKind::CompareRuntimeValues { right, .. }
        | SelectedInstructionKind::WriteRuntimeStorageBinary { right, .. }
        | SelectedInstructionKind::WriteRuntimePointeeBinary { right, .. }
        | SelectedInstructionKind::WriteRuntimeFrameIndexedBinary { right, .. } => Some(*right),
        _ => None,
    }
}
