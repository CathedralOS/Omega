use super::super::offsets::{
    runtime_text_buffer_materialize_target_address_offset,
    runtime_text_line_read_target_address_offset,
    runtime_text_literal_append_target_address_offset,
    runtime_text_stored_place_source_address_offset,
    runtime_text_stored_place_target_address_offset,
    runtime_text_stored_suffix_source_address_offset,
    runtime_text_stored_suffix_target_address_offset,
};
use super::context::InstructionRelocationContext;
use crate::storage_regions::storage_region_symbol_name;
use omega_target_program::SelectedInstructionKind;

pub(super) fn collect_runtime_text_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) {
    match instruction {
        SelectedInstructionKind::CompareRuntimeTextLiteral { buffer, .. } => {
            let buffer_symbol = &context.native_plan.data.objects.get(*buffer).symbol;
            context.insert_data_address_at_instruction_start(buffer_symbol);
        }
        SelectedInstructionKind::CompareRuntimeTextStorage {
            buffer,
            source_region,
            ..
        } => {
            let buffer_symbol = &context.native_plan.data.objects.get(*buffer).symbol;
            let source_symbol = storage_region_symbol_name(
                *source_region,
                context.native_plan.entry_machine_name(),
            );
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(8, &source_symbol);
        }
        SelectedInstructionKind::WriteRuntimeTextLiteral { buffer, .. } => {
            let buffer_symbol = &context.native_plan.data.objects.get(*buffer).symbol;
            context.insert_data_address_at_instruction_start(buffer_symbol);
        }
        SelectedInstructionKind::WriteRuntimeTextLiteralSegment { buffer, .. } => {
            let buffer_symbol = &context.native_plan.data.objects.get(*buffer).symbol;
            context.insert_data_address_at_instruction_start(buffer_symbol);
        }
        SelectedInstructionKind::AppendRuntimeTextStoredSuffix {
            buffer,
            source_region,
            target_region,
            ..
        } => {
            let buffer_symbol = &context.native_plan.data.objects.get(*buffer).symbol;
            let source_symbol = storage_region_symbol_name(
                *source_region,
                context.native_plan.entry_machine_name(),
            );
            let target_symbol = storage_region_symbol_name(
                *target_region,
                context.native_plan.entry_machine_name(),
            );
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_stored_suffix_source_address_offset(
                    context.native_plan.target.architecture,
                ),
                &source_symbol,
            );
            context.insert_data_address_at_relative_offset(
                runtime_text_stored_suffix_target_address_offset(
                    context.native_plan.target.architecture,
                ),
                &target_symbol,
            );
        }
        SelectedInstructionKind::AppendRuntimeTextStoredPlace {
            buffer,
            source_region,
            target_region,
            ..
        } => {
            let buffer_symbol = &context.native_plan.data.objects.get(*buffer).symbol;
            let source_symbol = storage_region_symbol_name(
                *source_region,
                context.native_plan.entry_machine_name(),
            );
            let target_symbol = storage_region_symbol_name(
                *target_region,
                context.native_plan.entry_machine_name(),
            );
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_stored_place_target_address_offset(
                    context.native_plan.target.architecture,
                ),
                &target_symbol,
            );
            context.insert_data_address_at_relative_offset(
                runtime_text_stored_place_source_address_offset(
                    context.native_plan.target.architecture,
                ),
                &source_symbol,
            );
        }
        SelectedInstructionKind::AppendRuntimeTextLiteral {
            buffer,
            target_region,
            ..
        } => {
            let buffer_symbol = &context.native_plan.data.objects.get(*buffer).symbol;
            let target_symbol = storage_region_symbol_name(
                *target_region,
                context.native_plan.entry_machine_name(),
            );
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_literal_append_target_address_offset(
                    context.native_plan.target.architecture,
                ),
                &target_symbol,
            );
        }
        SelectedInstructionKind::MaterializeRuntimeTextBuffer {
            buffer,
            target_region,
            ..
        } => {
            let buffer_symbol = &context.native_plan.data.objects.get(*buffer).symbol;
            let target_symbol = storage_region_symbol_name(
                *target_region,
                context.native_plan.entry_machine_name(),
            );
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_buffer_materialize_target_address_offset(
                    context.native_plan.target.architecture,
                ),
                &target_symbol,
            );
        }
        SelectedInstructionKind::ReadRuntimeTextLine {
            buffer,
            target_region,
            syscall_number,
            ..
        } => {
            let buffer_symbol = &context.native_plan.data.objects.get(*buffer).symbol;
            let target_symbol = storage_region_symbol_name(
                *target_region,
                context.native_plan.entry_machine_name(),
            );
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_line_read_target_address_offset(
                    context.native_plan.target.architecture,
                    *syscall_number,
                ),
                &target_symbol,
            );
        }
        _ => {}
    }
}
