use selected_instructions::{SelectedBlock, SelectedBlockId, SelectedFunction, SelectedTerminator};

use super::super::super::OptimizedResolvedSelectedFormLayoutError;
use super::super::super::SelectedFunctionLayoutPolicy;
use super::Fusion;

pub(super) fn derive<'a>(
    function: &'a SelectedFunction,
    fusion: Fusion<'_>,
    policy: SelectedFunctionLayoutPolicy,
) -> Result<Vec<&'a SelectedBlock>, OptimizedResolvedSelectedFormLayoutError> {
    let function_has_fusion = fusion.is_some_and(|fusion| {
        fusion
            .fusion()
            .plan()
            .actions
            .iter()
            .any(|action| action.machine == function.machine)
    });
    if let [block] = function.blocks.as_slice() {
        if function.entry_block != block.id
            || !matches!(block.terminator, SelectedTerminator::Return { .. })
        {
            return unsupported(function);
        }
        return Ok(vec![block]);
    }
    if !matches!(function.blocks.len(), 3 | 4) {
        return unsupported(function);
    }
    let entry = find(function, function.entry_block)?;
    let (taken, fallthrough) = match (&entry.terminator, policy) {
        (
            SelectedTerminator::ConditionalBranch {
                when_nonzero,
                when_zero,
                ..
            },
            SelectedFunctionLayoutPolicy::EntryThenZeroFallthroughThenNonzeroV1
            | SelectedFunctionLayoutPolicy::PerFunctionCanonicalShapeV1,
        ) => (when_nonzero, when_zero),
        (
            SelectedTerminator::ConditionalBranchU64LessThan {
                when_less,
                when_not_less,
                ..
            }
            | SelectedTerminator::ConditionalBranchI64LessThan {
                when_less,
                when_not_less,
                ..
            },
            SelectedFunctionLayoutPolicy::EntryThenNotLessFallthroughThenLessV1
            | SelectedFunctionLayoutPolicy::PerFunctionCanonicalShapeV1,
        ) if !function_has_fusion => (when_less, when_not_less),
        _ => return unsupported(function),
    };
    if taken.block == fallthrough.block || entry.id == taken.block || entry.id == fallthrough.block
    {
        return unsupported(function);
    }
    let fallthrough = find(function, fallthrough.block)?;
    let taken = find(function, taken.block)?;
    if function.blocks.len() == 4 {
        let (
            SelectedTerminator::Jump {
                successor: left, ..
            },
            SelectedTerminator::Jump {
                successor: right, ..
            },
        ) = (&fallthrough.terminator, &taken.terminator)
        else {
            return unsupported(function);
        };
        if left.block != right.block || [entry.id, fallthrough.id, taken.id].contains(&left.block) {
            return unsupported(function);
        }
        let joined = find(function, left.block)?;
        if !matches!(joined.terminator, SelectedTerminator::Return { .. }) {
            return unsupported(function);
        }
        return Ok(vec![entry, fallthrough, taken, joined]);
    }
    if !matches!(fallthrough.terminator, SelectedTerminator::Return { .. })
        || !matches!(taken.terminator, SelectedTerminator::Return { .. })
    {
        return unsupported(function);
    }
    Ok(vec![entry, fallthrough, taken])
}

fn find(
    function: &SelectedFunction,
    id: SelectedBlockId,
) -> Result<&SelectedBlock, OptimizedResolvedSelectedFormLayoutError> {
    function
        .blocks
        .iter()
        .find(|block| block.id == id)
        .ok_or(OptimizedResolvedSelectedFormLayoutError::UnsupportedFunctionShape(function.machine))
}

fn unsupported<T>(
    function: &SelectedFunction,
) -> Result<T, OptimizedResolvedSelectedFormLayoutError> {
    Err(OptimizedResolvedSelectedFormLayoutError::UnsupportedFunctionShape(function.machine))
}
