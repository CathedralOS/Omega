//! Input-only block layout. Backedges are layout edges, not ranking evidence.
use super::SelectedInstructionError;
use legalized_operations::{LegalizedScalarFunction, LegalizedScalarTerminator};

pub(super) fn derive(
    source: &LegalizedScalarFunction,
) -> Result<Vec<usize>, SelectedInstructionError> {
    let invalid = SelectedInstructionError::SourceCustodyMismatch;
    let entry = source
        .blocks
        .iter()
        .position(|block| block.id == source.entry_block)
        .ok_or(invalid.clone())?;
    let mut outgoing = Vec::new();
    for block in &source.blocks {
        let targets = match &block.terminator {
            LegalizedScalarTerminator::Return(_) => [None, None],
            LegalizedScalarTerminator::Jump { successor, .. } => [Some(successor.target), None],
            LegalizedScalarTerminator::Conditional {
                when_true,
                when_false,
                ..
            } => [Some(when_true.target), Some(when_false.target)],
        };
        outgoing.push(
            targets
                .into_iter()
                .flatten()
                .map(|target| {
                    source
                        .blocks
                        .iter()
                        .position(|candidate| candidate.id == target)
                        .ok_or(invalid.clone())
                })
                .collect::<Result<Vec<_>, _>>()?,
        );
    }
    // Iterative reverse postorder puts every dominating definition before its
    // uses, including when a predecessor is a backedge. No execution fuel is set.
    let mut visited = vec![false; source.blocks.len()];
    let mut pending = vec![(entry, false)];
    let mut postorder = Vec::new();
    while let Some((block, expanded)) = pending.pop() {
        if expanded {
            postorder.push(block);
            continue;
        }
        if visited[block] {
            continue;
        }
        visited[block] = true;
        pending.push((block, true));
        pending.extend(outgoing[block].iter().rev().map(|target| (*target, false)));
    }
    if postorder.len() != source.blocks.len() {
        return Err(invalid);
    }
    postorder.reverse();
    let mut order = vec![entry];
    if source.ranked.is_some() {
        order.extend((0..source.blocks.len()).filter(|index| *index != entry));
        return Ok(order);
    }
    while order.len() < source.blocks.len() {
        // Keep the established layout for acyclic inputs. At a cycle, use the
        // next reverse-postorder block instead of waiting for its own backedge.
        let next = (0..source.blocks.len())
            .find(|index| {
                !order.contains(index)
                    && outgoing.iter().enumerate().all(|(predecessor, targets)| {
                        !targets.contains(index) || order.contains(&predecessor)
                    })
            })
            .or_else(|| {
                postorder
                    .iter()
                    .copied()
                    .find(|index| !order.contains(index))
            })
            .ok_or(invalid.clone())?;
        order.push(next);
    }
    Ok(order)
}
