//! Independently reconstruct canonical fallthrough adjacency and complete coverage.
use super::super::super::{OptimizedResolvedSelectedFormLayoutError, SelectedFunctionLayoutPolicy};
use selected_instructions::{SelectedBlock, SelectedBlockId, SelectedFunction, SelectedTerminator};

pub(super) fn derive(
    function: &SelectedFunction,
    policy: SelectedFunctionLayoutPolicy,
) -> Result<Vec<&SelectedBlock>, OptimizedResolvedSelectedFormLayoutError> {
    let invalid =
        || OptimizedResolvedSelectedFormLayoutError::UnsupportedFunctionShape(function.machine);
    let entry = function
        .blocks
        .iter()
        .find(|block| block.id == function.entry_block)
        .ok_or_else(invalid)?;
    if policy == SelectedFunctionLayoutPolicy::SingleEntryBlockV1
        && (function.blocks.len() != 1
            || !matches!(entry.terminator, SelectedTerminator::Return { .. }))
    {
        return Err(invalid());
    }
    for (position, block) in function.blocks.iter().enumerate() {
        if function.blocks[..position]
            .iter()
            .any(|other| other.id == block.id)
        {
            return Err(invalid());
        }
        if let Some(destination) = fallthrough(block)
            && (destination == block.id
                || !function.blocks.iter().any(|other| other.id == destination))
        {
            return Err(invalid());
        }
        let incoming = function
            .blocks
            .iter()
            .filter(|source| fallthrough(source) == Some(block.id))
            .count();
        if incoming > 1 || (block.id == entry.id && incoming != 0) {
            return Err(invalid());
        }
    }
    let mut order = Vec::with_capacity(function.blocks.len());
    let mut next = Some(entry);
    while let Some(block) = next {
        if order
            .iter()
            .any(|previous: &&SelectedBlock| previous.id == block.id)
        {
            return Err(invalid());
        }
        order.push(block);
        next = if let Some(destination) = fallthrough(block) {
            Some(
                function
                    .blocks
                    .iter()
                    .find(|candidate| candidate.id == destination)
                    .ok_or_else(invalid)?,
            )
        } else {
            function.blocks.iter().find(|candidate| {
                !order.iter().any(|previous| previous.id == candidate.id)
                    && !function
                        .blocks
                        .iter()
                        .any(|source| fallthrough(source) == Some(candidate.id))
            })
        };
    }
    if order.len() != function.blocks.len() {
        return Err(invalid());
    }
    Ok(order)
}

fn fallthrough(block: &SelectedBlock) -> Option<SelectedBlockId> {
    match &block.terminator {
        SelectedTerminator::ConditionalBranch { when_zero, .. } => Some(when_zero.block),
        SelectedTerminator::ConditionalBranchU64LessThan { when_not_less, .. }
        | SelectedTerminator::ConditionalBranchI64LessThan { when_not_less, .. } => {
            Some(when_not_less.block)
        }
        SelectedTerminator::Jump { .. } | SelectedTerminator::Return { .. } => None,
    }
}
