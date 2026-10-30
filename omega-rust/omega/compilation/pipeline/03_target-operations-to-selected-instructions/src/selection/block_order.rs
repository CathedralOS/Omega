//! Input-only block layout. Backedges are layout edges, not ranking evidence.
use super::SelectedInstructionError;
use legalized_operations::{LegalizedScalarFunction, LegalizedScalarTerminator};

pub(super) fn derive(
    source: &LegalizedScalarFunction,
) -> Result<Vec<usize>, SelectedInstructionError> {
    let entry = source
        .block_index(source.entry_block)
        .ok_or(SelectedInstructionError::custody())?;
    let mut outgoing = Vec::new();
    for block in &source.blocks {
        let targets = match &block.terminator {
            LegalizedScalarTerminator::Crash { .. }
            | LegalizedScalarTerminator::InvokeNoNormal { .. }
            | LegalizedScalarTerminator::ExternalCompletion { .. }
            | LegalizedScalarTerminator::Return(_)
            | LegalizedScalarTerminator::ReturnCleanup { .. } => Vec::new(),
            LegalizedScalarTerminator::Jump { successor, .. } => vec![successor.target],
            LegalizedScalarTerminator::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true.target, when_false.target],
            LegalizedScalarTerminator::StructuralCase { cases, .. } => {
                cases.iter().map(|case| case.target).collect()
            }
        };
        outgoing.push(
            targets
                .into_iter()
                .map(|target| {
                    source
                        .block_index(target)
                        .ok_or(SelectedInstructionError::custody())
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
        return Err(SelectedInstructionError::custody());
    }
    postorder.reverse();
    let mut order = vec![entry];
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
            .ok_or(SelectedInstructionError::custody())?;
        order.push(next);
    }
    Ok(order)
}
