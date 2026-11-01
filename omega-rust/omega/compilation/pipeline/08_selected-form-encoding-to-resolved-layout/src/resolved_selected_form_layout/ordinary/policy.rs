use selected_instructions::{SelectedInstructionPlan, SelectedTerminator};

use super::super::OptimizedResolvedSelectedFormLayoutError as Error;
use super::super::{SelectedFunctionLayoutPolicy, is_admitted_canonical_shape, is_single_entry};

pub(in super::super) fn select(
    selected: &SelectedInstructionPlan,
) -> Result<SelectedFunctionLayoutPolicy, Error> {
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
                Some(selected_instructions::conditional_branch_less_pair_pattern!(..))
            )
        })
    {
        Ok(SelectedFunctionLayoutPolicy::EntryThenNotLessFallthroughThenLessV1)
    } else if selected.functions.iter().all(is_admitted_canonical_shape) {
        Ok(SelectedFunctionLayoutPolicy::PerFunctionCanonicalShapeV1)
    } else {
        Err(Error::UnsupportedFunctionShape(
            selected.functions[single_entry_count].machine,
        ))
    }
}
