use selected_instructions::{SelectedFunction, SelectedInstructionPlan, SelectedTerminator};

use super::super::{OptimizedResolvedSelectedFormLayoutError, SelectedFunctionLayoutPolicy};

pub(in super::super) fn select(
    selected: &SelectedInstructionPlan,
) -> Result<SelectedFunctionLayoutPolicy, OptimizedResolvedSelectedFormLayoutError> {
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
                Some(
                    SelectedTerminator::ConditionalBranchU64LessThan { .. }
                        | SelectedTerminator::ConditionalBranchI64LessThan { .. }
                )
            )
        })
    {
        Ok(SelectedFunctionLayoutPolicy::EntryThenNotLessFallthroughThenLessV1)
    } else if selected.functions.iter().all(is_admitted_canonical_shape) {
        Ok(SelectedFunctionLayoutPolicy::PerFunctionCanonicalShapeV1)
    } else {
        Err(
            OptimizedResolvedSelectedFormLayoutError::UnsupportedFunctionShape(
                selected.functions[single_entry_count].machine,
            ),
        )
    }
}

fn is_admitted_canonical_shape(function: &SelectedFunction) -> bool {
    is_single_entry(function)
        || (matches!(function.blocks.len(), 3 | 4)
            && matches!(
                function.blocks.first().map(|block| &block.terminator),
                Some(
                    SelectedTerminator::ConditionalBranch { .. }
                        | SelectedTerminator::ConditionalBranchU64LessThan { .. }
                        | SelectedTerminator::ConditionalBranchI64LessThan { .. }
                )
            ))
}

fn is_single_entry(function: &SelectedFunction) -> bool {
    let [block] = function.blocks.as_slice() else {
        return false;
    };
    function.entry_block == block.id
        && matches!(block.terminator, SelectedTerminator::Return { .. })
}
