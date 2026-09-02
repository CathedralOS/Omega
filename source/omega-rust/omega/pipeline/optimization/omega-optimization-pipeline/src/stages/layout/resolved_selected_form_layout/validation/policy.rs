use omega_selected_instructions::{SelectedFunction, SelectedInstructionPlan, SelectedTerminator};

use super::super::{OptimizedResolvedSelectedFormLayoutError, SelectedFunctionLayoutPolicy};

pub(super) fn derive(
    selected: &SelectedInstructionPlan,
) -> Result<SelectedFunctionLayoutPolicy, OptimizedResolvedSelectedFormLayoutError> {
    if !selected.structural_unit_functions.is_empty() {
        return Ok(SelectedFunctionLayoutPolicy::StructuralUnitCallThenReturnSingleEntryBlockV1);
    }
    let single_entry_count = selected
        .functions
        .iter()
        .filter(|function| is_single_entry(function))
        .count();
    if single_entry_count == selected.functions.len() {
        Ok(SelectedFunctionLayoutPolicy::SingleEntryBlockV1)
    } else if single_entry_count == 0
        && selected.functions.iter().all(|function| {
            matches!(
                function.blocks.first().map(|block| &block.terminator),
                Some(SelectedTerminator::ConditionalBranch { .. })
            )
        })
    {
        Ok(SelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1)
    } else if single_entry_count == 0
        && selected.functions.iter().all(|function| {
            matches!(
                function.blocks.first().map(|block| &block.terminator),
                Some(SelectedTerminator::ConditionalBranchU64LessThan { .. })
            )
        })
    {
        Ok(SelectedFunctionLayoutPolicy::EntryThenNotLessFallthroughThenLessV1)
    } else {
        Err(
            OptimizedResolvedSelectedFormLayoutError::UnsupportedFunctionShape(
                selected.functions[single_entry_count].machine,
            ),
        )
    }
}

fn is_single_entry(function: &SelectedFunction) -> bool {
    let [block] = function.blocks.as_slice() else {
        return false;
    };
    function.entry_block == block.id
        && matches!(block.terminator, SelectedTerminator::Return { .. })
}
