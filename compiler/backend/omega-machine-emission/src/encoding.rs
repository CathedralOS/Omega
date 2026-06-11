mod host;
mod runtime_storage;
mod runtime_text;

use crate::MachineEmissionContext;
use crate::layout::LaidOutMachineInstruction;
use crate::selected_instruction_queries::{selected_host_operation, selected_host_text_read};
use omega_assigned_target_operations::SelectedInstructionKind;
use omega_core::diagnostics::Diagnostic;

pub(super) fn encode_machine_instruction_bytes(
    input: MachineEmissionContext<'_>,
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
    kind: &SelectedInstructionKind,
) -> Result<Vec<u8>, Diagnostic> {
    if let Some(host_operation) = selected_host_operation(kind) {
        let Some(operands) = input
            .assigned_target_operations
            .instruction_operands(host_operation.operands)
        else {
            return Err(Diagnostic::error(
                "cannot encode host operation: missing operand span",
            ));
        };

        return host::encode_host_operation(input, host_operation.operation_key, operands);
    }

    match kind {
        SelectedInstructionKind::CompareRuntimeTextLiteral { literal, .. } => {
            runtime_text::encode_runtime_text_literal_compare(
                input,
                machine_instructions,
                machine_instruction_index,
                literal,
            )
        }
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
            is_float,
            ..
        } => runtime_storage::encode_runtime_storage_binary_write(
            input,
            *target_offset,
            *byte_size,
            *left,
            *operator,
            *right,
            *is_float,
        ),
        SelectedInstructionKind::WriteRuntimeStorageConvert {
            target_offset,
            target_byte_size,
            source,
            source_byte_size,
            source_is_float,
            target_is_float,
            source_signed,
            ..
        } => runtime_storage::encode_runtime_storage_convert(
            input,
            *target_offset,
            *target_byte_size,
            *source,
            *source_byte_size,
            *source_is_float,
            *target_is_float,
            *source_signed,
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
        SelectedInstructionKind::WriteRuntimeFrameBaseIndexedInteger {
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            value,
        } => runtime_storage::encode_runtime_frame_base_indexed_integer_write(
            input,
            *base_byte_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
            *byte_size,
            *value,
        ),
        SelectedInstructionKind::AppendWireLiteralByte {
            out_offset,
            written_offset,
            value,
            ..
        } => omega_instruction_selection::encode_append_wire_literal_byte(
            input.target.architecture,
            *out_offset,
            *written_offset,
            *value,
        ),
        SelectedInstructionKind::AppendWireScalarVarint {
            source_region,
            source_offset,
            byte_size,
            zigzag,
            out_offset,
            written_offset,
            ..
        } => omega_instruction_selection::encode_append_wire_scalar_varint(
            input.target.architecture,
            *source_region,
            *source_offset,
            *byte_size,
            *zigzag,
            *out_offset,
            *written_offset,
        ),
        SelectedInstructionKind::WriteRuntimeMachineIndexedInteger {
            base_byte_offset,
            index_region,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            value,
        } => runtime_storage::encode_runtime_machine_indexed_integer_write(
            input,
            *base_byte_offset,
            *index_region,
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
        SelectedInstructionKind::WriteRuntimeFrameBaseIndexedBinary {
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
        } => runtime_storage::encode_runtime_frame_base_indexed_binary_write(
            input,
            *base_byte_offset,
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
        SelectedInstructionKind::WriteRuntimeFrameString {
            byte_offset,
            byte_length,
            ..
        } => runtime_storage::encode_runtime_frame_string_write(input, *byte_offset, *byte_length),
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
        SelectedInstructionKind::WriteRuntimeFrameIndexedString {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_length,
            ..
        } => runtime_storage::encode_runtime_frame_indexed_string_write(
            input,
            *descriptor_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
            *byte_length,
        ),
        SelectedInstructionKind::WriteRuntimeMachineIndexedString {
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_length,
            ..
        } => runtime_storage::encode_runtime_machine_indexed_string_write(
            input,
            *base_byte_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
            *byte_length,
        ),
        SelectedInstructionKind::WriteRuntimeStorageAddressToRuntimeFrame {
            source_offset,
            target_offset,
            ..
        } => runtime_storage::encode_runtime_storage_address_to_runtime_frame_write(
            input,
            *source_offset,
            *target_offset,
        ),
        SelectedInstructionKind::WriteRuntimePointeeAddressToRuntimeFrame {
            pointer_byte_offset,
            field_byte_offset,
            target_offset,
        } => runtime_storage::encode_runtime_pointee_address_to_runtime_frame_write(
            input,
            *pointer_byte_offset,
            *field_byte_offset,
            *target_offset,
        ),
        SelectedInstructionKind::WriteRuntimeFrameIndexedAddressToRuntimeFrame {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            target_offset,
        } => runtime_storage::encode_runtime_frame_indexed_address_to_runtime_frame_write(
            input,
            *descriptor_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
            *target_offset,
        ),
        SelectedInstructionKind::WriteRuntimeFrameFixedIndexedAddressToRuntimeFrame {
            descriptor_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
            target_offset,
        } => runtime_storage::encode_runtime_frame_fixed_indexed_address_to_runtime_frame_write(
            input,
            *descriptor_offset,
            *element_index,
            *element_byte_size,
            *field_byte_offset,
            *target_offset,
        ),
        SelectedInstructionKind::WriteRuntimeFrameBaseIndexedAddressToRuntimeFrame {
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            target_offset,
        } => runtime_storage::encode_runtime_frame_base_indexed_address_to_runtime_frame_write(
            input,
            *base_byte_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
            *target_offset,
        ),
        SelectedInstructionKind::ReadRuntimeTextLine {
            ..
        } => {
            let Some(read) = selected_host_text_read(kind) else {
                return Err(Diagnostic::error(
                    "cannot encode runtime text read: missing host operation source",
                ));
            };
            runtime_text::encode_runtime_text_line_read(
                input,
                read.target_offset,
                read.byte_capacity,
                read.source,
            )
        }
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
        SelectedInstructionKind::CopyRuntimeFrameIndexedToRuntimeFrame {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
        } => runtime_storage::encode_runtime_storage_copy_from_runtime_frame_indexed(
            input,
            *descriptor_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
            *target_offset,
            *byte_count,
        ),
        SelectedInstructionKind::CopyRuntimeFrameIndexedToRuntimeStorage {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
            ..
        } => runtime_storage::encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_storage(
            input,
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
        } => runtime_storage::encode_runtime_storage_copy_from_runtime_frame_fixed_indexed(
            input,
            *descriptor_offset,
            *element_index,
            *element_byte_size,
            *field_byte_offset,
            *target_offset,
            *byte_count,
        ),
        SelectedInstructionKind::CopyRuntimeFrameFixedIndexedToRuntimeStorage {
            descriptor_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
            ..
        } => runtime_storage::encode_runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_storage(
            input,
            *descriptor_offset,
            *element_index,
            *element_byte_size,
            *field_byte_offset,
            *target_offset,
            *byte_count,
        ),
        SelectedInstructionKind::CopyRuntimeFrameFixedIndexedToRuntimePointee {
            descriptor_offset,
            element_index,
            element_byte_size,
            source_field_byte_offset,
            pointer_byte_offset,
            target_field_byte_offset,
            byte_count,
        } => runtime_storage::encode_runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_pointee(
            input,
            *descriptor_offset,
            *element_index,
            *element_byte_size,
            *source_field_byte_offset,
            *pointer_byte_offset,
            *target_field_byte_offset,
            *byte_count,
        ),
        SelectedInstructionKind::CopyRuntimeFrameIndexedToRuntimePointee {
            descriptor_offset,
            index_offset,
            element_byte_size,
            source_field_byte_offset,
            pointer_byte_offset,
            target_field_byte_offset,
            byte_count,
        } => runtime_storage::encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee(
            input,
            *descriptor_offset,
            *index_offset,
            *element_byte_size,
            *source_field_byte_offset,
            *pointer_byte_offset,
            *target_field_byte_offset,
            *byte_count,
        ),
        SelectedInstructionKind::CopyRuntimeMachineIndexedToRuntimeStorage {
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
            ..
        } => runtime_storage::encode_runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage(
            input,
            *base_byte_offset,
            *index_offset,
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
        } => runtime_storage::encode_runtime_storage_copy_to_runtime_pointee(
            input,
            *source_offset,
            *pointer_byte_offset,
            *field_byte_offset,
            *byte_count,
        ),
        SelectedInstructionKind::CopyRuntimePointeeToRuntimeFrame {
            pointer_byte_offset,
            field_byte_offset,
            target_offset,
            byte_count,
        } => runtime_storage::encode_runtime_storage_copy_from_runtime_pointee_to_runtime_frame(
            input,
            *pointer_byte_offset,
            *field_byte_offset,
            *target_offset,
            *byte_count,
        ),
        SelectedInstructionKind::EnterFunction
        | SelectedInstructionKind::EnterDispatchLoop { .. }
        | SelectedInstructionKind::EnterDispatchCase { .. }
        | SelectedInstructionKind::EvaluateDispatchGuard { .. }
        | SelectedInstructionKind::CompareRuntimeTextStorage { .. }
        | SelectedInstructionKind::CompareRuntimeStorage { .. }
        | SelectedInstructionKind::CompareRuntimeStorageValue { .. }
        | SelectedInstructionKind::SetDispatchState { .. }
        | SelectedInstructionKind::WriteReturnRegisterInteger { .. }
        | SelectedInstructionKind::TerminateDispatch
        | SelectedInstructionKind::LeaveDispatchCase
        | SelectedInstructionKind::LeaveDispatchLoop
        | SelectedInstructionKind::LeaveFunction
        | SelectedInstructionKind::BeginPlatformCall => Err(Diagnostic::error(
            "internal error: zero-width machine instruction reached byte encoder",
        )),
        SelectedInstructionKind::HostOperation { .. } => Err(Diagnostic::error(
            "internal error: host operation was not handled by machine encoder host query",
        )),
    }
}
