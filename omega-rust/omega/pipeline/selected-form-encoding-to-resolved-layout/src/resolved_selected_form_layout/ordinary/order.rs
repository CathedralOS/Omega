//! Place ordinary blocks by mandatory conditional fallthrough chains.
use super::super::{OptimizedResolvedSelectedFormLayoutError, SelectedFunctionLayoutPolicy};
use selected_instructions::{SelectedBlock, SelectedFunction, SelectedTerminator};

pub(super) fn derive(
    function: &SelectedFunction,
    policy: SelectedFunctionLayoutPolicy,
) -> Result<Vec<&SelectedBlock>, OptimizedResolvedSelectedFormLayoutError> {
    let invalid =
        || OptimizedResolvedSelectedFormLayoutError::UnsupportedFunctionShape(function.machine);
    let count = function.blocks.len();
    // Keep storage positions independent of external block identities. Sorting
    // this compact association once avoids scans per successor and does not
    // allocate up to the largest (possibly sparse) identifier.
    let mut positions: Vec<_> = function
        .blocks
        .iter()
        .enumerate()
        .map(|(position, block)| (block.id, position))
        .collect();
    if !positions.is_sorted_by_key(|(identity, _)| *identity) {
        positions.sort_unstable_by_key(|(identity, _)| *identity);
    }
    if positions.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(invalid());
    }
    let position_of = |identity| {
        positions
            .binary_search_by_key(&identity, |(identity, _)| *identity)
            .ok()
            .map(|position| positions[position].1)
    };
    let entry = position_of(function.entry_block).ok_or_else(invalid)?;
    if policy == SelectedFunctionLayoutPolicy::SingleEntryBlockV1
        && (count != 1
            || !matches!(
                function.blocks[entry].terminator,
                SelectedTerminator::Return { .. }
            ))
    {
        return Err(invalid());
    }
    let mut following = vec![None; count];
    let mut preceding = vec![None; count];
    for (source, block) in function.blocks.iter().enumerate() {
        let successor = match &block.terminator {
            SelectedTerminator::ConditionalBranch { when_zero, .. } => Some(when_zero),
            SelectedTerminator::ConditionalBranchU64LessThan { when_not_less, .. }
            | SelectedTerminator::ConditionalBranchI64LessThan { when_not_less, .. } => {
                Some(when_not_less)
            }
            SelectedTerminator::Jump { .. }
            | SelectedTerminator::Return { .. }
            | SelectedTerminator::HostedExitProcess { .. } => None,
        };
        if let Some(successor) = successor {
            let destination = position_of(successor.block).ok_or_else(invalid)?;
            if destination == source || preceding[destination].replace(source).is_some() {
                return Err(invalid());
            }
            following[source] = Some(destination);
        }
    }
    // Entry cannot be placed after another block's fallthrough. Explicit Jump
    // edges impose no adjacency and may point backward, including loop backedges.
    if preceding[entry].is_some() {
        return Err(invalid());
    }
    let roots = std::iter::once(entry)
        .chain((0..count).filter(|index| *index != entry && preceding[*index].is_none()));
    let mut visited = vec![false; count];
    let mut order = Vec::with_capacity(count);
    for root in roots {
        let mut current = Some(root);
        while let Some(index) = current {
            if std::mem::replace(&mut visited[index], true) {
                return Err(invalid());
            }
            order.push(&function.blocks[index]);
            current = following[index];
        }
    }
    if order.len() != count {
        return Err(invalid());
    }
    Ok(order)
}

#[cfg(test)]
mod tests;
