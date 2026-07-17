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
        SelectedInstructionKind::WriteRuntimeStorageConvert {
            target_offset,
            target_byte_size,
            source,
            source_byte_size,
            source_is_float,
            target_is_float,
            source_signed,
            trapping,
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
            *trapping,
        ),
        SelectedInstructionKind::AtomicFetchAdd {
            target_offset,
            byte_size,
            delta,
            ..
        } => runtime_storage::encode_atomic_fetch_add(
            input,
            *target_offset,
            *byte_size,
            *delta,
        ),
        SelectedInstructionKind::AtomicCompareExchange {
            target_offset,
            byte_size,
            expected,
            new_value,
            ..
        } => runtime_storage::encode_atomic_compare_exchange(
            input,
            *target_offset,
            *byte_size,
            *expected,
            *new_value,
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
        SelectedInstructionKind::AppendWireTextBytes {
            source_region,
            source_offset,
            out_offset,
            out_length,
            written_offset,
            ..
        } => omega_instruction_selection::encode_append_wire_text_bytes(
            input.target.architecture,
            *source_region,
            *source_offset,
            *out_offset,
            *out_length,
            *written_offset,
        ),
        SelectedInstructionKind::ReadWireExpectedByte {
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            expected,
            ..
        } => omega_instruction_selection::encode_read_wire_expected_byte(
            input.target.architecture,
            *buffer_offset,
            *buffer_length,
            *read_offset,
            *ok_offset,
            *expected,
        ),
        SelectedInstructionKind::ReadWireScalarVarint {
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            target_region,
            target_offset,
            byte_size,
            zigzag,
            ..
        } => omega_instruction_selection::encode_read_wire_scalar_varint(
            input.target.architecture,
            *buffer_offset,
            *buffer_length,
            *read_offset,
            *ok_offset,
            *target_region,
            *target_offset,
            *byte_size,
            *zigzag,
        ),
        SelectedInstructionKind::ReadWireByteSlice {
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            target_region,
            target_offset,
            predicate_mask,
            ..
        } => omega_instruction_selection::encode_read_wire_byte_slice(
            input.target.architecture,
            *buffer_offset,
            *buffer_length,
            *read_offset,
            *ok_offset,
            *target_region,
            *target_offset,
            *predicate_mask,
        ),
        SelectedInstructionKind::ReadWireNestedOpen {
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            end_offset,
            ..
        } => omega_instruction_selection::encode_read_wire_nested_open(
            input.target.architecture,
            *buffer_offset,
            *buffer_length,
            *read_offset,
            *ok_offset,
            *end_offset,
        ),
        SelectedInstructionKind::ReadWireNestedClose {
            buffer_offset,
            read_offset,
            ok_offset,
            end_offset,
            ..
        } => omega_instruction_selection::encode_read_wire_nested_close(
            input.target.architecture,
            *buffer_offset,
            *read_offset,
            *ok_offset,
            *end_offset,
        ),
        SelectedInstructionKind::AppendWireRepeatedScalarVarint {
            source_region,
            source_offset,
            byte_size,
            zigzag,
            index,
            count_region,
            count_offset,
            out_offset,
            written_offset,
            ..
        } => omega_instruction_selection::encode_append_wire_repeated_scalar_varint(
            input.target.architecture,
            *source_region,
            *source_offset,
            *byte_size,
            *zigzag,
            *index,
            *count_region,
            *count_offset,
            *out_offset,
            *written_offset,
        ),
        SelectedInstructionKind::ReadWireRepeatedScalarVarint {
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            end_offset,
            count_region,
            count_offset,
            target_region,
            target_offset,
            byte_size,
            zigzag,
            ..
        } => omega_instruction_selection::encode_read_wire_repeated_scalar_varint(
            input.target.architecture,
            *buffer_offset,
            *buffer_length,
            *read_offset,
            *ok_offset,
            *end_offset,
            *count_region,
            *count_offset,
            *target_region,
            *target_offset,
            *byte_size,
            *zigzag,
        ),
        SelectedInstructionKind::AppendRuntimeMachineBoundedBufferSource {
            target_byte_offset,
            source_byte_offset,
            source_in_frame,
        } => runtime_storage::encode_runtime_machine_bounded_buffer_source_append(
            input,
            *target_byte_offset,
            *source_byte_offset,
            *source_in_frame,
        ),
        SelectedInstructionKind::AppendRuntimeMachineBoundedBufferLiteral {
            target_byte_offset,
            literal,
        } => runtime_storage::encode_runtime_machine_bounded_buffer_literal_append(
            input,
            *target_byte_offset,
            literal,
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
                read.is_bounded_buffer,
            )
        }
        SelectedInstructionKind::ReadRuntimeByte {
            target_offset,
            payload_offset,
            source,
            ..
        } => runtime_text::encode_runtime_byte_read(input, *target_offset, *payload_offset, source),
        SelectedInstructionKind::WriteRuntimeByte {
            source_offset,
            source,
            source_is_place,
            ..
        } => {
            // A literal source relocates the adrp pair to the 1-byte data
            // object, whose byte sits at offset 0.
            let offset = if *source_is_place { *source_offset } else { 0 };
            runtime_text::encode_runtime_byte_write(input, offset, source)
        }
        SelectedInstructionKind::CopyPlaces {
            source,
            target,
            byte_count,
        } => omega_instruction_selection::encode_copy_places(
            input.target.architecture,
            source,
            target,
            *byte_count,
        ),
        SelectedInstructionKind::WritePlaceInteger {
            target,
            value,
            byte_size,
        } => omega_instruction_selection::encode_write_place_integer(
            input.target.architecture,
            target,
            *value,
            *byte_size,
        ),
        SelectedInstructionKind::WritePlaceString {
            target,
            byte_length,
            ..
        } => omega_instruction_selection::encode_write_place_string(
            input.target.architecture,
            target,
            *byte_length,
        ),
        SelectedInstructionKind::WritePlaceBoundedBuffer { target, literal } => {
            omega_instruction_selection::encode_write_place_bounded_buffer(
                input.target.architecture,
                target,
                literal,
            )
        }
        SelectedInstructionKind::WritePlaceAddress {
            source,
            target_offset,
        } => omega_instruction_selection::encode_write_place_address(
            input.target.architecture,
            source,
            *target_offset,
        ),
        SelectedInstructionKind::WritePlaceBinary {
            target,
            byte_size,
            left,
            operator,
            right,
            is_float,
            domain,
            target_signed,
        } => runtime_storage::encode_write_place_binary(
            input,
            target,
            *byte_size,
            *left,
            *operator,
            *right,
            *is_float,
            *domain,
            *target_signed,
        ),
        SelectedInstructionKind::MachineHalt => Ok(
            omega_instruction_selection::encode_machine_halt_bytes(input.target.architecture),
        ),
        // Port I/O (`asm { out .. }` / `asm { in .. }`) has no branch distance;
        // its storage operands carry relocations, applied by omega-relocations
        // against the offsets pinned in the ISA encoders.
        SelectedInstructionKind::PortWrite { port, value } => {
            omega_instruction_selection::encode_port_write_bytes(
                input.assigned_target_operations,
                *port,
                *value,
            )
        }
        SelectedInstructionKind::PortRead {
            port,
            dest_byte_offset,
            ..
        } => omega_instruction_selection::encode_port_read_bytes(
            input.assigned_target_operations,
            *port,
            *dest_byte_offset,
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
        | SelectedInstructionKind::CopyRuntimeStorageToReturnRegister { .. }
        | SelectedInstructionKind::WriteEntryArgumentRegister { .. }
        | SelectedInstructionKind::WriteEntryArgumentsSliceDescriptor { .. }
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
