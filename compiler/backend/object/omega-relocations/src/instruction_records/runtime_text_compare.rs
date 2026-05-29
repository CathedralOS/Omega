use super::context::InstructionRelocationContext;
use omega_target_operations::SelectedInstructionKind;

pub(super) fn collect_runtime_text_compare_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) -> bool {
    match instruction {
        SelectedInstructionKind::CompareRuntimeTextLiteral { buffer, .. } => {
            let buffer_symbol = context.data_object_symbol_handle(*buffer);
            context.insert_data_address_at_instruction_start(buffer_symbol);
            true
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
            true
        }
        _ => false,
    }
}
