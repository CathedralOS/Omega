use super::super::offsets::{
    wire_decode_byte_slice_target_page_offset, wire_decode_nested_end_page_offset,
    wire_decode_ok_page_offset, wire_decode_read_page_offset,
    wire_decode_repeated_count_page_offset, wire_decode_repeated_target_page_offset,
    wire_decode_varint_target_page_offset,
};
use super::context::InstructionRelocationContext;
use omega_target_operations::SelectedInstructionKind;

/// compact_binary v0 wire reads: the BUFFER page is materialized at the
/// instruction start, the READ (cursor) page after the buffer-offset add, the
/// OK (sticky flag) page after the full shared prologue, and the varint's
/// TARGET page after the read loop. The offsets come from the same width
/// functions the encoders assert against, keeping the relocation records in
/// lockstep with the emitted bytes.
pub(super) fn collect_wire_decode_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) -> bool {
    match instruction {
        SelectedInstructionKind::ReadWireExpectedByte {
            buffer_region,
            buffer_offset,
            read_region,
            read_offset,
            ok_region,
            ..
        } => {
            context.insert_data_address_at_instruction_start(
                context.storage_region_symbol_handle(*buffer_region),
            );
            context.insert_data_address_at_relative_offset(
                wire_decode_read_page_offset(context.input.target.architecture, *buffer_offset),
                context.storage_region_symbol_handle(*read_region),
            );
            context.insert_data_address_at_relative_offset(
                wire_decode_ok_page_offset(
                    context.input.target.architecture,
                    *buffer_offset,
                    *read_offset,
                ),
                context.storage_region_symbol_handle(*ok_region),
            );
            true
        }
        SelectedInstructionKind::ReadWireScalarVarint {
            buffer_region,
            buffer_offset,
            buffer_length,
            read_region,
            read_offset,
            ok_region,
            target_region,
            zigzag,
            ..
        } => {
            context.insert_data_address_at_instruction_start(
                context.storage_region_symbol_handle(*buffer_region),
            );
            context.insert_data_address_at_relative_offset(
                wire_decode_read_page_offset(context.input.target.architecture, *buffer_offset),
                context.storage_region_symbol_handle(*read_region),
            );
            context.insert_data_address_at_relative_offset(
                wire_decode_ok_page_offset(
                    context.input.target.architecture,
                    *buffer_offset,
                    *read_offset,
                ),
                context.storage_region_symbol_handle(*ok_region),
            );
            context.insert_data_address_at_relative_offset(
                wire_decode_varint_target_page_offset(
                    context.input.target.architecture,
                    *buffer_offset,
                    *buffer_length,
                    *read_offset,
                    *zigzag,
                ),
                context.storage_region_symbol_handle(*target_region),
            );
            true
        }
        SelectedInstructionKind::ReadWireByteSlice {
            buffer_region,
            buffer_offset,
            buffer_length,
            read_region,
            read_offset,
            ok_region,
            target_region,
            predicate_mask,
            ..
        } => {
            context.insert_data_address_at_instruction_start(
                context.storage_region_symbol_handle(*buffer_region),
            );
            context.insert_data_address_at_relative_offset(
                wire_decode_read_page_offset(context.input.target.architecture, *buffer_offset),
                context.storage_region_symbol_handle(*read_region),
            );
            context.insert_data_address_at_relative_offset(
                wire_decode_ok_page_offset(
                    context.input.target.architecture,
                    *buffer_offset,
                    *read_offset,
                ),
                context.storage_region_symbol_handle(*ok_region),
            );
            context.insert_data_address_at_relative_offset(
                wire_decode_byte_slice_target_page_offset(
                    context.input.target.architecture,
                    *buffer_offset,
                    *buffer_length,
                    *read_offset,
                    *predicate_mask,
                ),
                context.storage_region_symbol_handle(*target_region),
            );
            true
        }
        // The nested open/close share the decode prologue exactly, so the
        // buffer/read/ok pages sit at the same offsets; the END-slot page is
        // materialized right after the prologue.
        SelectedInstructionKind::ReadWireNestedOpen {
            buffer_region,
            buffer_offset,
            read_region,
            read_offset,
            ok_region,
            end_region,
            ..
        }
        | SelectedInstructionKind::ReadWireNestedClose {
            buffer_region,
            buffer_offset,
            read_region,
            read_offset,
            ok_region,
            end_region,
            ..
        } => {
            context.insert_data_address_at_instruction_start(
                context.storage_region_symbol_handle(*buffer_region),
            );
            context.insert_data_address_at_relative_offset(
                wire_decode_read_page_offset(context.input.target.architecture, *buffer_offset),
                context.storage_region_symbol_handle(*read_region),
            );
            context.insert_data_address_at_relative_offset(
                wire_decode_ok_page_offset(
                    context.input.target.architecture,
                    *buffer_offset,
                    *read_offset,
                ),
                context.storage_region_symbol_handle(*ok_region),
            );
            context.insert_data_address_at_relative_offset(
                wire_decode_nested_end_page_offset(
                    context.input.target.architecture,
                    *buffer_offset,
                    *read_offset,
                ),
                context.storage_region_symbol_handle(*end_region),
            );
            true
        }
        // The repeated (guarded) element read shares the decode prologue and
        // the nested END-page position, then materializes the TARGET page
        // after the read loop and the COUNT page after the target store.
        SelectedInstructionKind::ReadWireRepeatedScalarVarint {
            buffer_region,
            buffer_offset,
            buffer_length,
            read_region,
            read_offset,
            ok_region,
            end_region,
            end_offset,
            count_region,
            target_region,
            target_offset,
            byte_size,
            zigzag,
            range,
            ..
        } => {
            context.insert_data_address_at_instruction_start(
                context.storage_region_symbol_handle(*buffer_region),
            );
            context.insert_data_address_at_relative_offset(
                wire_decode_read_page_offset(context.input.target.architecture, *buffer_offset),
                context.storage_region_symbol_handle(*read_region),
            );
            context.insert_data_address_at_relative_offset(
                wire_decode_ok_page_offset(
                    context.input.target.architecture,
                    *buffer_offset,
                    *read_offset,
                ),
                context.storage_region_symbol_handle(*ok_region),
            );
            context.insert_data_address_at_relative_offset(
                wire_decode_nested_end_page_offset(
                    context.input.target.architecture,
                    *buffer_offset,
                    *read_offset,
                ),
                context.storage_region_symbol_handle(*end_region),
            );
            context.insert_data_address_at_relative_offset(
                wire_decode_repeated_target_page_offset(
                    context.input.target.architecture,
                    *buffer_offset,
                    *buffer_length,
                    *read_offset,
                    *end_offset,
                    *zigzag,
                ),
                context.storage_region_symbol_handle(*target_region),
            );
            context.insert_data_address_at_relative_offset(
                wire_decode_repeated_count_page_offset(
                    context.input.target.architecture,
                    *buffer_offset,
                    *buffer_length,
                    *read_offset,
                    *end_offset,
                    *target_offset,
                    *byte_size,
                    *zigzag,
                    *range,
                ),
                context.storage_region_symbol_handle(*count_region),
            );
            true
        }
        _ => false,
    }
}
