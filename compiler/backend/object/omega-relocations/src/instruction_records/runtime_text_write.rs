use super::context::InstructionRelocationContext;
use omega_target_operations::SelectedInstructionKind;

pub(super) fn collect_runtime_text_write_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) -> bool {
    match instruction {
        SelectedInstructionKind::WriteRuntimeTextLiteral { buffer, .. }
        | SelectedInstructionKind::WriteRuntimeTextLiteralSegment { buffer, .. } => {
            let buffer_symbol = context.data_object_symbol_handle(*buffer);
            context.insert_data_address_at_instruction_start(buffer_symbol);
            true
        }
        _ => false,
    }
}
