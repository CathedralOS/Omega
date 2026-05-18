mod dispatch;
mod host;
mod runtime_storage;
mod runtime_text;

use crate::MachineEmissionContext;
use crate::layout::LaidOutMachineInstruction;
use omega_core::diagnostics::Diagnostic;
use omega_target_operations::{SelectedInstructionKind, StateGuardLowering, StateGuardOperator};

pub(super) fn encode_machine_instruction_bytes(
    input: MachineEmissionContext<'_>,
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
    kind: &SelectedInstructionKind,
) -> Result<Vec<u8>, Diagnostic> {
    match kind {
        SelectedInstructionKind::HostOperation {
            operation_key,
            operands,
        } => {
            let Some(operands) = input.instructions.operands.span(*operands) else {
                return Err(Diagnostic::error(
                    "cannot encode host operation: missing operand span",
                ));
            };

            host::encode_host_operation(input, *operation_key, operands)
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
        } => dispatch::encode_dispatch_guard_compare_static(
            input,
            machine_instructions,
            machine_instruction_index,
            *byte_offset,
            *byte_size,
            *expected_value,
            *operator,
        ),
        SelectedInstructionKind::CompareRuntimeTextLiteral { literal, .. } => {
            runtime_text::encode_runtime_text_literal_compare(
                input,
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
            input,
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
            input,
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
            input,
            machine_instructions,
            machine_instruction_index,
            *byte_offset,
            *byte_size,
            *expected_value,
            *operator,
        ),
        SelectedInstructionKind::CompareRuntimeValues {
            left,
            right,
            byte_size,
            operator,
        } => runtime_storage::encode_runtime_value_compare(
            input,
            machine_instructions,
            machine_instruction_index,
            *left,
            *right,
            *byte_size,
            *operator,
        ),
        SelectedInstructionKind::WriteRuntimeTextLiteral { literal, .. } => {
            runtime_text::encode_runtime_text_literal_write(input, literal)
        }
        SelectedInstructionKind::WriteRuntimeTextLiteralSegment {
            byte_offset,
            literal,
            ..
        } => runtime_text::encode_runtime_text_literal_segment_write(input, *byte_offset, literal),
        SelectedInstructionKind::AppendRuntimeTextStoredSuffix {
            buffer_offset,
            source_offset,
            target_offset,
            length_delta,
            ..
        } => runtime_text::encode_runtime_text_stored_suffix_append(
            input,
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
            input,
            *source_offset,
            *target_offset,
        ),
        SelectedInstructionKind::AppendRuntimeTextStoredPlaceToRuntimePointee {
            source_offset,
            pointer_byte_offset,
            field_byte_offset,
            ..
        } => runtime_text::encode_runtime_text_stored_place_append_to_runtime_pointee(
            input,
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
        } => runtime_text::encode_runtime_text_stored_place_append_to_runtime_frame_indexed(
            input,
            *source_offset,
            *descriptor_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
        ),
        SelectedInstructionKind::AppendRuntimeTextLiteral {
            target_offset,
            literal,
            ..
        } => runtime_text::encode_runtime_text_literal_append(input, *target_offset, literal),
        SelectedInstructionKind::AppendRuntimeTextLiteralToRuntimePointee {
            pointer_byte_offset,
            field_byte_offset,
            literal,
            ..
        } => runtime_text::encode_runtime_text_literal_append_to_runtime_pointee(
            input,
            *pointer_byte_offset,
            *field_byte_offset,
            literal,
        ),
        SelectedInstructionKind::AppendRuntimeTextLiteralToRuntimeFrameIndexed {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            literal,
            ..
        } => runtime_text::encode_runtime_text_literal_append_to_runtime_frame_indexed(
            input,
            *descriptor_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
            literal,
        ),
        SelectedInstructionKind::MaterializeRuntimeTextBuffer { target_offset, .. } => {
            runtime_text::encode_runtime_text_buffer_materialize(input, *target_offset)
        }
        SelectedInstructionKind::MaterializeRuntimeTextBufferToRuntimePointee {
            pointer_byte_offset,
            field_byte_offset,
            ..
        } => runtime_text::encode_runtime_text_buffer_materialize_to_runtime_pointee(
            input,
            *pointer_byte_offset,
            *field_byte_offset,
        ),
        SelectedInstructionKind::MaterializeRuntimeTextBufferToRuntimeFrameIndexed {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            ..
        } => runtime_text::encode_runtime_text_buffer_materialize_to_runtime_frame_indexed(
            input,
            *descriptor_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
        ),
        SelectedInstructionKind::WriteRuntimeMachineInteger {
            byte_offset,
            byte_size,
            value,
        } => runtime_storage::encode_runtime_machine_integer_write(
            input,
            *byte_offset,
            *byte_size,
            *value,
        ),
        SelectedInstructionKind::WriteRuntimeStorageInteger {
            byte_offset,
            byte_size,
            value,
            ..
        } => runtime_storage::encode_runtime_machine_integer_write(
            input,
            *byte_offset,
            *byte_size,
            *value,
        ),
        SelectedInstructionKind::WriteRuntimePointeeInteger {
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
            value,
        } => runtime_storage::encode_runtime_pointee_integer_write(
            input,
            *pointer_byte_offset,
            *field_byte_offset,
            *byte_size,
            *value,
        ),
        SelectedInstructionKind::WriteRuntimeStorageBinary {
            target_offset,
            byte_size,
            left,
            operator,
            right,
            ..
        } => runtime_storage::encode_runtime_storage_binary_write(
            input,
            *target_offset,
            *byte_size,
            *left,
            *operator,
            *right,
        ),
        SelectedInstructionKind::WriteRuntimePointeeBinary {
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
        } => runtime_storage::encode_runtime_pointee_binary_write(
            input,
            *pointer_byte_offset,
            *field_byte_offset,
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
        } => runtime_storage::encode_runtime_frame_indexed_integer_write(
            input,
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
        } => runtime_storage::encode_runtime_frame_indexed_binary_write(
            input,
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
        } => {
            runtime_storage::encode_runtime_machine_string_write(input, *byte_offset, *byte_length)
        }
        SelectedInstructionKind::WriteRuntimePointeeString {
            pointer_byte_offset,
            field_byte_offset,
            byte_length,
            ..
        } => runtime_storage::encode_runtime_pointee_string_write(
            input,
            *pointer_byte_offset,
            *field_byte_offset,
            *byte_length,
        ),
        SelectedInstructionKind::ReadRuntimeTextLine {
            target_offset,
            byte_capacity,
            source,
            ..
        } => runtime_text::encode_runtime_text_line_read(
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
        } => runtime_storage::encode_runtime_storage_copy(
            input,
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
        } => runtime_storage::encode_runtime_storage_copy_to_runtime_frame_indexed(
            input,
            *source_offset,
            *descriptor_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
            *byte_count,
        ),
        SelectedInstructionKind::CopyRuntimeStorageToRuntimePointee {
            source_offset,
            pointer_byte_offset,
            field_byte_offset,
            byte_count,
            ..
        } => runtime_storage::encode_runtime_storage_copy_to_runtime_pointee(
            input,
            *source_offset,
            *pointer_byte_offset,
            *field_byte_offset,
            *byte_count,
        ),
        SelectedInstructionKind::EnterFunction
        | SelectedInstructionKind::EnterDispatchLoop { .. }
        | SelectedInstructionKind::EnterDispatchCase { .. }
        | SelectedInstructionKind::EvaluateDispatchGuard { .. }
        | SelectedInstructionKind::SetDispatchState { .. }
        | SelectedInstructionKind::TerminateDispatch
        | SelectedInstructionKind::LeaveDispatchCase
        | SelectedInstructionKind::LeaveDispatchLoop
        | SelectedInstructionKind::LeaveFunction
        | SelectedInstructionKind::BeginPlatformCall => Err(Diagnostic::error(
            "internal error: zero-width machine instruction reached byte encoder",
        )),
    }
}
