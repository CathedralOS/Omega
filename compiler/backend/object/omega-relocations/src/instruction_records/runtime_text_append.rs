use super::super::offsets::{
    runtime_text_indexed_literal_append_buffer_address_offset,
    runtime_text_indexed_stored_place_buffer_address_offset,
    runtime_text_indexed_stored_place_source_address_offset,
    runtime_text_literal_append_target_address_offset,
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
        SelectedInstructionKind::AppendRuntimeTextStoredPlace {
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
                runtime_text_stored_place_target_address_offset(context.input.target.architecture),
                target_symbol,
            );
            context.insert_data_address_at_relative_offset(
                runtime_text_stored_place_source_address_offset(context.input.target.architecture),
                source_symbol,
            );
            true
        }
        SelectedInstructionKind::AppendRuntimeTextStoredPlaceToRuntimePointee {
            buffer,
            source_region,
            ..
        } => {
            let buffer_symbol = context.data_object_symbol_handle(*buffer);
            let source_symbol = context.storage_region_symbol_handle(*source_region);
            let target_symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_stored_place_target_address_offset(context.input.target.architecture),
                target_symbol,
            );
            context.insert_data_address_at_relative_offset(
                runtime_text_stored_place_source_address_offset(context.input.target.architecture),
                source_symbol,
            );
            true
        }
        SelectedInstructionKind::AppendRuntimeTextStoredPlaceToRuntimeFrameIndexed {
            buffer,
            source_region,
            element_byte_size,
            field_byte_offset,
            ..
        } => {
            let buffer_symbol = context.data_object_symbol_handle(*buffer);
            let source_symbol = context.storage_region_symbol_handle(*source_region);
            let target_symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address_at_instruction_start(target_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_indexed_stored_place_buffer_address_offset(
                    context.input.target.architecture,
                    *element_byte_size,
                    *field_byte_offset,
                ),
                buffer_symbol,
            );
            context.insert_data_address_at_relative_offset(
                runtime_text_indexed_stored_place_source_address_offset(
                    context.input.target.architecture,
                    *element_byte_size,
                    *field_byte_offset,
                ),
                source_symbol,
            );
            true
        }
        SelectedInstructionKind::AppendRuntimeTextLiteral {
            buffer,
            target_region,
            ..
        } => {
            let buffer_symbol = context.data_object_symbol_handle(*buffer);
            let target_symbol = context.storage_region_symbol_handle(*target_region);
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_literal_append_target_address_offset(
                    context.input.target.architecture,
                ),
                target_symbol,
            );
            true
        }
        SelectedInstructionKind::AppendRuntimeTextLiteralToRuntimePointee { buffer, .. } => {
            let buffer_symbol = context.data_object_symbol_handle(*buffer);
            let target_symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_literal_append_target_address_offset(
                    context.input.target.architecture,
                ),
                target_symbol,
            );
            true
        }
        SelectedInstructionKind::AppendRuntimeTextLiteralToRuntimeFrameIndexed {
            buffer,
            element_byte_size,
            field_byte_offset,
            ..
        } => {
            let buffer_symbol = context.data_object_symbol_handle(*buffer);
            let target_symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address_at_instruction_start(target_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_indexed_literal_append_buffer_address_offset(
                    context.input.target.architecture,
                    *element_byte_size,
                    *field_byte_offset,
                ),
                buffer_symbol,
            );
            true
        }
        _ => false,
    }
}
