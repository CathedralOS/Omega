use super::super::offsets::{
    runtime_text_indexed_literal_append_buffer_address_offset,
    runtime_text_indexed_stored_place_buffer_address_offset,
    runtime_text_indexed_stored_place_source_address_offset,
    runtime_text_literal_append_target_address_offset,
    runtime_text_stored_place_pointee_source_address_offset,
    runtime_text_stored_place_source_address_offset,
    runtime_text_stored_place_target_address_offset,
    runtime_text_stored_suffix_source_address_offset,
    runtime_text_stored_suffix_target_address_offset,
};
use super::context::InstructionRelocationContext;
use omega_target_operations::SelectedInstructionKind;

pub(super) fn collect_runtime_text_append_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) -> bool {
    match instruction {
        SelectedInstructionKind::AppendRuntimeTextStoredSuffix {
            buffer,
            source_region,
            target_region,
            ..
        } => {
            let buffer_symbol = context.data_object_symbol_handle(*buffer);
            let source_symbol = context.storage_region_symbol_handle(*source_region);
            let target_symbol = context.storage_region_symbol_handle(*target_region);
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_stored_suffix_source_address_offset(context.input.target.architecture),
                source_symbol,
            );
            context.insert_data_address_at_relative_offset(
                runtime_text_stored_suffix_target_address_offset(context.input.target.architecture),
                target_symbol,
            );
            true
        }
        // Task #132: the place-shaped survivors decompose by shape.
        SelectedInstructionKind::AppendTextStoredToPlace {
            buffer,
            source_region,
            target,
            ..
        } => {
            let buffer_symbol = context.data_object_symbol_handle(*buffer);
            let source_symbol = context.storage_region_symbol_handle(*source_region);
            match omega_instruction_selection::classify_write_place_shape(target) {
                omega_instruction_selection::WritePlaceShape::Direct { .. } => {
                    context.insert_data_address_at_instruction_start(buffer_symbol);
                    context.insert_data_address_at_relative_offset(
                        runtime_text_stored_place_target_address_offset(
                            context.input.target.architecture,
                        ),
                        context.storage_region_symbol_handle(target.region),
                    );
                    context.insert_data_address_at_relative_offset(
                        runtime_text_stored_place_source_address_offset(
                            context.input.target.architecture,
                        ),
                        source_symbol,
                    );
                }
                omega_instruction_selection::WritePlaceShape::Pointee {
                    pointer_byte_offset,
                    field_byte_offset,
                } => {
                    context.insert_data_address_at_instruction_start(buffer_symbol);
                    context.insert_data_address_at_relative_offset(
                        runtime_text_stored_place_target_address_offset(
                            context.input.target.architecture,
                        ),
                        context.storage_region_symbol_handle(target.region),
                    );
                    context.insert_data_address_at_relative_offset(
                        runtime_text_stored_place_pointee_source_address_offset(
                            context.input.target.architecture,
                            pointer_byte_offset,
                            field_byte_offset,
                        ),
                        source_symbol,
                    );
                }
                omega_instruction_selection::WritePlaceShape::FrameIndexed {
                    index_byte_size,
                    element_byte_size,
                    field_byte_offset,
                    ..
                } => {
                    context.insert_data_address_at_instruction_start(
                        context.storage_region_symbol_handle(target.region),
                    );
                    context.insert_data_address_at_relative_offset(
                        runtime_text_indexed_stored_place_buffer_address_offset(
                            context.input.target.architecture,
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                        ),
                        buffer_symbol,
                    );
                    context.insert_data_address_at_relative_offset(
                        runtime_text_indexed_stored_place_source_address_offset(
                            context.input.target.architecture,
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                        ),
                        source_symbol,
                    );
                }
                _ => unreachable!(
                    "an unsupported AppendTextStoredToPlace shape refuses at encoding; \
                     layout would have failed first"
                ),
            }
            true
        }
        SelectedInstructionKind::AppendTextLiteralToPlace { buffer, target, .. } => {
            let buffer_symbol = context.data_object_symbol_handle(*buffer);
            match omega_instruction_selection::classify_write_place_shape(target) {
                omega_instruction_selection::WritePlaceShape::Direct { .. }
                | omega_instruction_selection::WritePlaceShape::Pointee { .. } => {
                    context.insert_data_address_at_instruction_start(buffer_symbol);
                    context.insert_data_address_at_relative_offset(
                        runtime_text_literal_append_target_address_offset(
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
                        runtime_text_indexed_literal_append_buffer_address_offset(
                            context.input.target.architecture,
                            element_byte_size,
                            field_byte_offset,
                        ),
                        buffer_symbol,
                    );
                }
                _ => unreachable!(
                    "an unsupported AppendTextLiteralToPlace shape refuses at encoding; \
                     layout would have failed first"
                ),
            }
            true
        }
        _ => false,
    }
}
