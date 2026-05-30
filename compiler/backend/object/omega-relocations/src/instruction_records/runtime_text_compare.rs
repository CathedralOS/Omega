use super::context::InstructionRelocationContext;
use omega_instruction_selection::{
    runtime_text_literal_compare_width, runtime_text_storage_compare_width,
};
use omega_target_operations::SelectedInstructionKind;

pub(super) fn collect_runtime_text_compare_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) -> bool {
    match instruction {
        SelectedInstructionKind::CompareRuntimeTextLiteral { buffer, literal, .. } => {
            // Only emit the buffer-address relocation when the comparison actually occupies
            // bytes (and therefore carries an inline 64-bit buffer-address immediate). On targets
            // where the comparison folds into the following `DispatchCaseEnter` and emits zero
            // bytes, anchoring here would land the 8-byte address on the next instruction.
            if runtime_text_literal_compare_width(context.input.target.architecture, literal) != 0 {
                let buffer_symbol = context.data_object_symbol_handle(*buffer);
                context.insert_data_address_at_instruction_start(buffer_symbol);
            }
            true
        }
        SelectedInstructionKind::CompareRuntimeTextStorage {
            buffer,
            source_region,
            ..
        } => {
            // Same zero-byte-lowering hazard as the dispatch guard: on x86_64 a
            // `RuntimeTextStorageCompare` folds into the following `DispatchCaseEnter` and emits no
            // bytes (runtime_text_storage_compare_width == 0). It carries no buffer/source address
            // immediates to relocate, and its text offset coincides with the next instruction
            // (a `movabs r15`/`mov r12d` storage load), so emitting these two Absolute64
            // relocations would splatter 8-byte addresses across that instruction and crash with
            // `0xC0000005`. Only anchor when the comparison occupies real bytes.
            if runtime_text_storage_compare_width(context.input.target.architecture) != 0 {
                let buffer_symbol = context.data_object_symbol_handle(*buffer);
                let source_symbol = context.storage_region_symbol_handle(*source_region);
                context.insert_data_address_at_instruction_start(buffer_symbol);
                context.insert_data_address_at_relative_offset(8, source_symbol);
            }
            true
        }
        _ => false,
    }
}
