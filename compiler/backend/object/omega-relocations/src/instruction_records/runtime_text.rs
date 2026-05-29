use super::context::InstructionRelocationContext;
use super::runtime_text_append;
use super::runtime_text_materialize;
use super::runtime_text_read;
use omega_target_operations::SelectedInstructionKind;

pub(super) fn collect_runtime_text_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) {
    if runtime_text_append::collect_runtime_text_append_relocations(context, instruction) {
        return;
    }
    if runtime_text_materialize::collect_runtime_text_materialize_relocations(context, instruction)
    {
        return;
    }

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
        SelectedInstructionKind::ReadRuntimeTextLine { .. } => {
            runtime_text_read::collect_runtime_text_read_relocations(context, instruction)
        }
        _ => {}
    }
}
