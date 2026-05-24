use super::super::offsets::{
    runtime_text_buffer_materialize_target_address_offset,
    runtime_text_indexed_buffer_materialize_buffer_address_offset,
    runtime_text_indexed_literal_append_buffer_address_offset,
    runtime_text_indexed_stored_place_buffer_address_offset,
    runtime_text_indexed_stored_place_source_address_offset,
    runtime_text_line_read_import_call_offset, runtime_text_line_read_target_address_offset,
    runtime_text_literal_append_target_address_offset,
    runtime_text_stored_place_source_address_offset,
    runtime_text_stored_place_target_address_offset,
    runtime_text_stored_suffix_source_address_offset,
    runtime_text_stored_suffix_target_address_offset,
};
use super::context::InstructionRelocationContext;
use omega_calling_conventions::HostBindingMechanism;
use omega_object_file::{RelocationRecord, object_symbol_handle_by_name};
use omega_target_operations::{RuntimeTextReadSource, SelectedInstructionKind};

pub(super) fn collect_runtime_text_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) {
    match instruction {
        SelectedInstructionKind::CompareRuntimeTextLiteral { buffer, .. } => {
            let buffer_symbol = context.data_object_symbol_handle(*buffer);
            context.insert_data_address_at_instruction_start(buffer_symbol);
        }
        SelectedInstructionKind::CompareRuntimeTextStorage {
            buffer,
            source_region,
            ..
        } => {
            let buffer_symbol = context.data_object_symbol_handle(*buffer);
            let source_symbol = context.storage_region_symbol_handle(*source_region);
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(8, source_symbol);
        }
        SelectedInstructionKind::WriteRuntimeTextLiteral { buffer, .. } => {
            let buffer_symbol = context.data_object_symbol_handle(*buffer);
            context.insert_data_address_at_instruction_start(buffer_symbol);
        }
        SelectedInstructionKind::WriteRuntimeTextLiteralSegment { buffer, .. } => {
            let buffer_symbol = context.data_object_symbol_handle(*buffer);
            context.insert_data_address_at_instruction_start(buffer_symbol);
        }
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
        }
        SelectedInstructionKind::MaterializeRuntimeTextBuffer {
            buffer,
            target_region,
            ..
        } => {
            let buffer_symbol = context.data_object_symbol_handle(*buffer);
            let target_symbol = context.storage_region_symbol_handle(*target_region);
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_buffer_materialize_target_address_offset(
                    context.input.target.architecture,
                ),
                target_symbol,
            );
        }
        SelectedInstructionKind::MaterializeRuntimeTextBufferToRuntimePointee {
            buffer, ..
        } => {
            let buffer_symbol = context.data_object_symbol_handle(*buffer);
            let target_symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_buffer_materialize_target_address_offset(
                    context.input.target.architecture,
                ),
                target_symbol,
            );
        }
        SelectedInstructionKind::MaterializeRuntimeTextBufferToRuntimeFrameIndexed {
            buffer,
            element_byte_size,
            field_byte_offset,
            ..
        } => {
            let buffer_symbol = context.data_object_symbol_handle(*buffer);
            let target_symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address_at_instruction_start(target_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_indexed_buffer_materialize_buffer_address_offset(
                    context.input.target.architecture,
                    *element_byte_size,
                    *field_byte_offset,
                ),
                buffer_symbol,
            );
        }
        SelectedInstructionKind::ReadRuntimeTextLine {
            buffer,
            target_region,
            source,
            ..
        } => {
            let RuntimeTextReadSource::HostOperation { operation_key } = source;
            let Some(binding) = context.input.instructions.host_binding(*operation_key) else {
                return;
            };
            let buffer_symbol = context.data_object_symbol_handle(*buffer);
            let target_symbol = context.storage_region_symbol_handle(*target_region);
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_line_read_target_address_offset(
                    context.input.target.architecture,
                    &binding.mechanism,
                ),
                target_symbol,
            );
            if let HostBindingMechanism::Import { symbol, .. } = &binding.mechanism {
                context.relocation_plan.records.insert(RelocationRecord {
                    function_symbol_handle: context.function_symbol_handle,
                    selected_instruction_index: context.selected_instruction_index,
                    text_offset: runtime_text_line_read_import_call_offset(
                        context.input.target.architecture,
                        context.selected_text_offset,
                    ),
                    byte_width: 4,
                    symbol_handle: object_symbol_handle_by_name(
                        &context.input.object,
                        symbol.as_ref(),
                    ),
                    kind: super::super::offsets::external_call_relocation_kind(
                        context.input.target.architecture,
                    ),
                });
            }
        }
        _ => {}
    }
}
