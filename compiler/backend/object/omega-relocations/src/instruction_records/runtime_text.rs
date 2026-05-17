use super::super::offsets::{
    runtime_text_buffer_materialize_target_address_offset,
    runtime_text_line_read_import_call_offset, runtime_text_line_read_target_address_offset,
    runtime_text_literal_append_target_address_offset,
    runtime_text_stored_place_source_address_offset,
    runtime_text_stored_place_target_address_offset,
    runtime_text_stored_suffix_source_address_offset,
    runtime_text_stored_suffix_target_address_offset,
};
use super::context::InstructionRelocationContext;
use omega_object::{RelocationRecord, object_symbol_handle_by_name, storage_region_symbol_name};
use omega_target_operations::{RuntimeTextReadSource, SelectedInstructionKind};

pub(super) fn collect_runtime_text_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) {
    match instruction {
        SelectedInstructionKind::CompareRuntimeTextLiteral { buffer, .. } => {
            let buffer_symbol = &context.input.data.objects.get(*buffer).symbol;
            context.insert_data_address_at_instruction_start(buffer_symbol);
        }
        SelectedInstructionKind::CompareRuntimeTextStorage {
            buffer,
            source_region,
            ..
        } => {
            let buffer_symbol = &context.input.data.objects.get(*buffer).symbol;
            let source_symbol =
                storage_region_symbol_name(*source_region, context.input.entry_machine_name);
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(8, &source_symbol);
        }
        SelectedInstructionKind::WriteRuntimeTextLiteral { buffer, .. } => {
            let buffer_symbol = &context.input.data.objects.get(*buffer).symbol;
            context.insert_data_address_at_instruction_start(buffer_symbol);
        }
        SelectedInstructionKind::WriteRuntimeTextLiteralSegment { buffer, .. } => {
            let buffer_symbol = &context.input.data.objects.get(*buffer).symbol;
            context.insert_data_address_at_instruction_start(buffer_symbol);
        }
        SelectedInstructionKind::AppendRuntimeTextStoredSuffix {
            buffer,
            source_region,
            target_region,
            ..
        } => {
            let buffer_symbol = &context.input.data.objects.get(*buffer).symbol;
            let source_symbol =
                storage_region_symbol_name(*source_region, context.input.entry_machine_name);
            let target_symbol =
                storage_region_symbol_name(*target_region, context.input.entry_machine_name);
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_stored_suffix_source_address_offset(context.input.target.architecture),
                &source_symbol,
            );
            context.insert_data_address_at_relative_offset(
                runtime_text_stored_suffix_target_address_offset(context.input.target.architecture),
                &target_symbol,
            );
        }
        SelectedInstructionKind::AppendRuntimeTextStoredPlace {
            buffer,
            source_region,
            target_region,
            ..
        } => {
            let buffer_symbol = &context.input.data.objects.get(*buffer).symbol;
            let source_symbol =
                storage_region_symbol_name(*source_region, context.input.entry_machine_name);
            let target_symbol =
                storage_region_symbol_name(*target_region, context.input.entry_machine_name);
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_stored_place_target_address_offset(context.input.target.architecture),
                &target_symbol,
            );
            context.insert_data_address_at_relative_offset(
                runtime_text_stored_place_source_address_offset(context.input.target.architecture),
                &source_symbol,
            );
        }
        SelectedInstructionKind::AppendRuntimeTextStoredPlaceToRuntimePointee {
            buffer,
            source_region,
            ..
        } => {
            let buffer_symbol = &context.input.data.objects.get(*buffer).symbol;
            let source_symbol =
                storage_region_symbol_name(*source_region, context.input.entry_machine_name);
            let target_symbol =
                storage_region_symbol_name(omega_target_operations::RuntimeStorageRegion::RuntimeFrame, context.input.entry_machine_name);
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_stored_place_target_address_offset(context.input.target.architecture),
                &target_symbol,
            );
            context.insert_data_address_at_relative_offset(
                runtime_text_stored_place_source_address_offset(context.input.target.architecture),
                &source_symbol,
            );
        }
        SelectedInstructionKind::AppendRuntimeTextStoredPlaceToRuntimeFrameIndexed {
            buffer,
            source_region,
            ..
        } => {
            let buffer_symbol = &context.input.data.objects.get(*buffer).symbol;
            let source_symbol =
                storage_region_symbol_name(*source_region, context.input.entry_machine_name);
            let target_symbol =
                storage_region_symbol_name(omega_target_operations::RuntimeStorageRegion::RuntimeFrame, context.input.entry_machine_name);
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_stored_place_target_address_offset(context.input.target.architecture),
                &target_symbol,
            );
            context.insert_data_address_at_relative_offset(
                runtime_text_stored_place_source_address_offset(context.input.target.architecture),
                &source_symbol,
            );
        }
        SelectedInstructionKind::AppendRuntimeTextLiteral {
            buffer,
            target_region,
            ..
        } => {
            let buffer_symbol = &context.input.data.objects.get(*buffer).symbol;
            let target_symbol =
                storage_region_symbol_name(*target_region, context.input.entry_machine_name);
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_literal_append_target_address_offset(
                    context.input.target.architecture,
                ),
                &target_symbol,
            );
        }
        SelectedInstructionKind::AppendRuntimeTextLiteralToRuntimePointee {
            buffer,
            ..
        } => {
            let buffer_symbol = &context.input.data.objects.get(*buffer).symbol;
            let target_symbol =
                storage_region_symbol_name(omega_target_operations::RuntimeStorageRegion::RuntimeFrame, context.input.entry_machine_name);
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_literal_append_target_address_offset(
                    context.input.target.architecture,
                ),
                &target_symbol,
            );
        }
        SelectedInstructionKind::AppendRuntimeTextLiteralToRuntimeFrameIndexed {
            buffer,
            ..
        } => {
            let buffer_symbol = &context.input.data.objects.get(*buffer).symbol;
            let target_symbol =
                storage_region_symbol_name(omega_target_operations::RuntimeStorageRegion::RuntimeFrame, context.input.entry_machine_name);
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_literal_append_target_address_offset(
                    context.input.target.architecture,
                ),
                &target_symbol,
            );
        }
        SelectedInstructionKind::MaterializeRuntimeTextBuffer {
            buffer,
            target_region,
            ..
        } => {
            let buffer_symbol = &context.input.data.objects.get(*buffer).symbol;
            let target_symbol =
                storage_region_symbol_name(*target_region, context.input.entry_machine_name);
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_buffer_materialize_target_address_offset(
                    context.input.target.architecture,
                ),
                &target_symbol,
            );
        }
        SelectedInstructionKind::MaterializeRuntimeTextBufferToRuntimePointee {
            buffer,
            ..
        } => {
            let buffer_symbol = &context.input.data.objects.get(*buffer).symbol;
            let target_symbol =
                storage_region_symbol_name(omega_target_operations::RuntimeStorageRegion::RuntimeFrame, context.input.entry_machine_name);
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_buffer_materialize_target_address_offset(
                    context.input.target.architecture,
                ),
                &target_symbol,
            );
        }
        SelectedInstructionKind::MaterializeRuntimeTextBufferToRuntimeFrameIndexed {
            buffer,
            ..
        } => {
            let buffer_symbol = &context.input.data.objects.get(*buffer).symbol;
            let target_symbol =
                storage_region_symbol_name(omega_target_operations::RuntimeStorageRegion::RuntimeFrame, context.input.entry_machine_name);
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_buffer_materialize_target_address_offset(
                    context.input.target.architecture,
                ),
                &target_symbol,
            );
        }
        SelectedInstructionKind::ReadRuntimeTextLine {
            buffer,
            target_region,
            source,
            ..
        } => {
            let buffer_symbol = &context.input.data.objects.get(*buffer).symbol;
            let target_symbol =
                storage_region_symbol_name(*target_region, context.input.entry_machine_name);
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_line_read_target_address_offset(
                    context.input.target.architecture,
                    source,
                ),
                &target_symbol,
            );
            if let RuntimeTextReadSource::Import { symbol } = source {
                context.relocation_plan.records.insert(RelocationRecord {
                    function_symbol: context.function.symbol.to_string(),
                    selected_instruction_index: context.selected_instruction_index,
                    text_offset: runtime_text_line_read_import_call_offset(
                        context.input.target.architecture,
                        context.selected_text_offset,
                    ),
                    byte_width: 4,
                    symbol: symbol.to_string(),
                    symbol_handle: object_symbol_handle_by_name(&context.input.object, symbol.as_ref()),
                    kind: super::super::offsets::external_call_relocation_kind(
                        context.input.target.architecture,
                    ),
                });
            }
        }
        _ => {}
    }
}
