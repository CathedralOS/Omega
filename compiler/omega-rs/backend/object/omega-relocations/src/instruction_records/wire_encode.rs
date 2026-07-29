use super::super::offsets::{
    wire_append_repeated_count_page_offset, wire_append_repeated_source_page_offset,
    wire_append_varint_source_page_offset, wire_append_written_page_offset,
};
use super::context::InstructionRelocationContext;
use omega_target_operations::SelectedInstructionKind;

/// compact_binary v0 wire appends: the OUT buffer page is materialized at the
/// instruction start, the WRITTEN (cursor) page after the out-offset add, and
/// the varint's SOURCE page after the full shared prologue. The offsets come
/// from the same width functions the encoders assert against, keeping the
/// relocation records in lockstep with the emitted bytes.
pub(super) fn collect_wire_encode_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) -> bool {
    match instruction {
        SelectedInstructionKind::AppendWireLiteralByte {
            out_region,
            out_offset,
            written_region,
            ..
        } => {
            context.insert_data_address_at_instruction_start(
                context.storage_region_symbol_handle(*out_region),
            );
            context.insert_data_address_at_relative_offset(
                wire_append_written_page_offset(context.input.target.architecture, *out_offset),
                context.storage_region_symbol_handle(*written_region),
            );
            true
        }
        SelectedInstructionKind::AppendWireScalarVarint {
            source_region,
            out_region,
            out_offset,
            written_region,
            written_offset,
            ..
        }
        // The text-bytes append materializes its source (descriptor) page at
        // the same post-prologue position as the varint's scalar page.
        | SelectedInstructionKind::AppendWireTextBytes {
            source_region,
            out_region,
            out_offset,
            written_region,
            written_offset,
            ..
        }
        | SelectedInstructionKind::AppendWireScalarSlice {
            source_region,
            out_region,
            out_offset,
            written_region,
            written_offset,
            ..
        } => {
            context.insert_data_address_at_instruction_start(
                context.storage_region_symbol_handle(*out_region),
            );
            context.insert_data_address_at_relative_offset(
                wire_append_written_page_offset(context.input.target.architecture, *out_offset),
                context.storage_region_symbol_handle(*written_region),
            );
            context.insert_data_address_at_relative_offset(
                wire_append_varint_source_page_offset(
                    context.input.target.architecture,
                    *out_offset,
                    *written_offset,
                ),
                context.storage_region_symbol_handle(*source_region),
            );
            true
        }
        // The repeated (guarded) append shares the out/written prologue, then
        // materializes the COUNT page right after it and the element's SOURCE
        // page after the guard block.
        SelectedInstructionKind::AppendWireRepeatedScalarVarint {
            source_region,
            index,
            count_region,
            count_offset,
            out_region,
            out_offset,
            written_region,
            written_offset,
            ..
        } => {
            context.insert_data_address_at_instruction_start(
                context.storage_region_symbol_handle(*out_region),
            );
            context.insert_data_address_at_relative_offset(
                wire_append_written_page_offset(context.input.target.architecture, *out_offset),
                context.storage_region_symbol_handle(*written_region),
            );
            context.insert_data_address_at_relative_offset(
                wire_append_repeated_count_page_offset(
                    context.input.target.architecture,
                    *out_offset,
                    *written_offset,
                ),
                context.storage_region_symbol_handle(*count_region),
            );
            context.insert_data_address_at_relative_offset(
                wire_append_repeated_source_page_offset(
                    context.input.target.architecture,
                    *out_offset,
                    *written_offset,
                    *count_offset,
                    *index,
                ),
                context.storage_region_symbol_handle(*source_region),
            );
            true
        }
        _ => false,
    }
}
