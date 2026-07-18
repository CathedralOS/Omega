use super::super::offsets::{
    runtime_text_buffer_materialize_target_address_offset,
    runtime_text_indexed_buffer_materialize_buffer_address_offset,
};
use super::context::InstructionRelocationContext;
use omega_target_operations::SelectedInstructionKind;

pub(super) fn collect_runtime_text_materialize_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) -> bool {
    match instruction {
        // Task #132: the place-shaped survivor decomposes by the SAME
        // classifier the encoder uses -- direct/pointee share the buffer@0 +
        // target-at-offset layout (the pointee target base is the frame);
        // frame-indexed flips to frame@0 + buffer at the indexed offset.
        SelectedInstructionKind::MaterializeTextBufferToPlace { buffer, target } => {
            let buffer_symbol = context.data_object_symbol_handle(*buffer);
            match omega_instruction_selection::classify_write_place_shape(target) {
                omega_instruction_selection::WritePlaceShape::Direct { .. }
                | omega_instruction_selection::WritePlaceShape::Pointee { .. } => {
                    context.insert_data_address_at_instruction_start(buffer_symbol);
                    context.insert_data_address_at_relative_offset(
                        runtime_text_buffer_materialize_target_address_offset(
                            context.input.target.architecture,
                        ),
                        context.storage_region_symbol_handle(target.region),
                    );
                }
                omega_instruction_selection::WritePlaceShape::FrameIndexed {
                    element_byte_size,
                    field_byte_offset,
                    ..
                } => {
                    context.insert_data_address_at_instruction_start(
                        context.storage_region_symbol_handle(target.region),
                    );
                    context.insert_data_address_at_relative_offset(
                        runtime_text_indexed_buffer_materialize_buffer_address_offset(
                            context.input.target.architecture,
                            element_byte_size,
                            field_byte_offset,
                        ),
                        buffer_symbol,
                    );
                }
                _ => unreachable!(
                    "an unsupported MaterializeTextBufferToPlace shape refuses at \
                     encoding; layout would have failed first"
                ),
            }
            true
        }
        _ => false,
    }
}
