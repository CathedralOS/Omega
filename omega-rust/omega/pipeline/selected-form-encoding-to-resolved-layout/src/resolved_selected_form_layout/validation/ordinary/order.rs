//! Independently reconstruct canonical fallthrough adjacency and complete coverage.
use super::super::super::{OptimizedResolvedSelectedFormLayoutError, SelectedFunctionLayoutPolicy};
use selected_instructions::{SelectedBlock, SelectedBlockId, SelectedFunction, SelectedTerminator};
use std::collections::BTreeMap;

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
    let mut positions = BTreeMap::new();
    for (position, block) in function.blocks.iter().enumerate() {
        if positions.insert(block.id, position).is_some() {
            return Err(invalid());
        }
    }
    let mut has_incoming = vec![false; function.blocks.len()];
    for block in &function.blocks {
        if let Some(destination) = fallthrough(block) {
            let position = *positions.get(&destination).ok_or_else(invalid)?;
            if destination == block.id
                || destination == entry.id
                || std::mem::replace(&mut has_incoming[position], true)
            {
                return Err(invalid());
            }
        }
    }
    let mut roots = function
        .blocks
        .iter()
        .enumerate()
        .filter_map(|(position, block)| {
            (!has_incoming[position] && block.id != entry.id).then_some(block)
        });
    let mut visited = vec![false; function.blocks.len()];
    let mut order = Vec::with_capacity(function.blocks.len());
    let mut next = Some(entry);
    while let Some(block) = next {
        let position = positions[&block.id];
        if std::mem::replace(&mut visited[position], true) {
            return Err(invalid());
        }
        order.push(block);
        next = if let Some(destination) = fallthrough(block) {
            Some(&function.blocks[*positions.get(&destination).ok_or_else(invalid)?])
        } else {
            roots.next()
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
        SelectedTerminator::Jump { .. }
        | SelectedTerminator::Return { .. }
        | SelectedTerminator::HostedExitProcess { .. } => None,
    }
}

// Exercise independent reconstruction against the same raw graph expectations,
// never against the producer's answer.
#[cfg(test)]
#[allow(clippy::duplicate_mod)] // The suite binds `super::derive` to this independent checker.
#[path = "../../ordinary/order/tests.rs"]
mod tests;
