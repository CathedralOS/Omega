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
use crate::instructions::SelectedInstructionKind;

pub(super) fn collect_runtime_text_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) {
    match instruction {
        SelectedInstructionKind::CompareRuntimeTextLiteral { buffer_symbol, .. } => {
            context.insert_data_address_at_instruction_start(buffer_symbol);
        }
        SelectedInstructionKind::CompareRuntimeTextStorage {
            buffer_symbol,
            source_symbol,
            ..
        } => {
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(8, source_symbol);
        }
        SelectedInstructionKind::WriteRuntimeTextLiteral { buffer_symbol, .. } => {
            context.insert_data_address_at_instruction_start(buffer_symbol);
        }
        SelectedInstructionKind::WriteRuntimeTextLiteralSegment { buffer_symbol, .. } => {
            context.insert_data_address_at_instruction_start(buffer_symbol);
        }
        SelectedInstructionKind::AppendRuntimeTextStoredSuffix {
            buffer_symbol,
            source_symbol,
            target_symbol,
            ..
        } => {
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_stored_suffix_source_address_offset(
                    context.native_plan.target.architecture,
                ),
                source_symbol,
            );
            context.insert_data_address_at_relative_offset(
                runtime_text_stored_suffix_target_address_offset(
                    context.native_plan.target.architecture,
                ),
                target_symbol,
            );
        }
        SelectedInstructionKind::AppendRuntimeTextStoredPlace {
            buffer_symbol,
            source_symbol,
            target_symbol,
            ..
        } => {
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_stored_place_target_address_offset(
                    context.native_plan.target.architecture,
                ),
                target_symbol,
            );
            context.insert_data_address_at_relative_offset(
                runtime_text_stored_place_source_address_offset(
                    context.native_plan.target.architecture,
                ),
                source_symbol,
            );
        }
        SelectedInstructionKind::AppendRuntimeTextLiteral {
            buffer_symbol,
            target_symbol,
            ..
        } => {
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_literal_append_target_address_offset(
                    context.native_plan.target.architecture,
                ),
                target_symbol,
            );
        }
        SelectedInstructionKind::MaterializeRuntimeTextBuffer {
            buffer_symbol,
            target_symbol,
            ..
        } => {
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_buffer_materialize_target_address_offset(
                    context.native_plan.target.architecture,
                ),
                target_symbol,
            );
        }
        SelectedInstructionKind::ReadRuntimeTextLine {
            buffer_symbol,
            target_symbol,
            syscall_number,
            ..
        } => {
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_line_read_target_address_offset(
                    context.native_plan.target.architecture,
                    *syscall_number,
                ),
                target_symbol,
            );
        }
        _ => {}
    }
}
