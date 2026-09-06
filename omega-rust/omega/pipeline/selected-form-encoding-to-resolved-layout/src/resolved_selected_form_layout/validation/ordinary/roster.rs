use std::collections::BTreeMap;

use selected_instructions::{
    SelectedBlock, SelectedFunction, SelectedInstruction, SelectedInstructionId, SelectedTerminator,
};

use post_allocation_machine_to_selected_form_encoding::SelectedFormEncodingRow;

use super::super::super::OptimizedResolvedSelectedFormLayoutError;
use super::PreLayoutRows;

pub(super) fn collect_pre_layout_rows<'a>(
    function: &SelectedFunction,
    rows: &mut PreLayoutRows<'a>,
) -> Result<
    BTreeMap<SelectedInstructionId, &'a SelectedFormEncodingRow>,
    OptimizedResolvedSelectedFormLayoutError,
> {
    let mut collected = BTreeMap::new();
    for block in &function.blocks {
        for instruction in instructions(block) {
            let row = rows.next().ok_or(
                OptimizedResolvedSelectedFormLayoutError::MissingInstruction(instruction.id),
            )?;
            if row.instruction != instruction.id {
                return Err(
                    OptimizedResolvedSelectedFormLayoutError::MissingInstruction(instruction.id),
                );
            }
            if collected.insert(instruction.id, row).is_some() {
                return Err(
                    OptimizedResolvedSelectedFormLayoutError::DuplicateInstruction(instruction.id),
                );
            }
        }
    }
    Ok(collected)
}

pub(super) fn instructions(block: &SelectedBlock) -> Vec<&SelectedInstruction> {
    block
        .instructions
        .iter()
        .chain(std::iter::once(match &block.terminator {
            SelectedTerminator::ConditionalBranch { instruction, .. }
            | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
            | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
            | SelectedTerminator::Jump { instruction, .. }
            | SelectedTerminator::Return { instruction, .. } => instruction,
        }))
        .collect()
}
