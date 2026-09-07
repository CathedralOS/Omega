//! Visit every ownership transfer, including cycles and their outgoing paths.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{BlockId, EdgeId};
use terminal_psi::{Block, Terminator};

/// Keep the established topological order for acyclic graphs and the ranked
/// skeleton. Otherwise visit from entry, so every block has at least one
/// incoming frontier before it is processed. The caller must check arrivals
/// left after traversal against the previously established block entries.
pub(super) fn block_order(
    entry: BlockId,
    blocks: &BTreeMap<BlockId, &Block>,
    representation_backedges: &BTreeSet<EdgeId>,
) -> Vec<BlockId> {
    let mut successors = BTreeMap::<BlockId, Vec<BlockId>>::new();
    let mut predecessors = blocks
        .keys()
        .map(|block| (*block, 0_usize))
        .collect::<BTreeMap<_, _>>();
    for block in blocks.values() {
        let targets = match &block.terminator {
            Terminator::Jump { edge, target, .. } => vec![(*edge, *target)],
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => vec![
                (when_true.edge, when_true.target),
                (when_false.edge, when_false.target),
            ],
            Terminator::StructuralCase { cases, .. } => {
                cases.iter().map(|case| (case.edge, case.target)).collect()
            }
            Terminator::Return { .. }
            | Terminator::ReturnUnit { .. }
            | Terminator::ReturnUnitPartialAffine { .. }
            | Terminator::ReturnUnitNominalAffine { .. }
            | Terminator::ReturnStructural { .. }
            | Terminator::Crash { .. } => Vec::new(),
        };
        let targets = targets
            .into_iter()
            .filter_map(|(edge, target)| {
                (!representation_backedges.contains(&edge)).then_some(target)
            })
            .collect::<Vec<_>>();
        for target in &targets {
            *predecessors
                .get_mut(target)
                .expect("control validation established every target") += 1;
        }
        successors.insert(block.id, targets);
    }
    let mut ready = predecessors
        .iter()
        .filter_map(|(block, count)| (*count == 0).then_some(*block))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(blocks.len());
    while let Some(block) = ready.pop_first() {
        order.push(block);
        for target in &successors[&block] {
            let count = predecessors
                .get_mut(target)
                .expect("control validation established every target");
            *count -= 1;
            if *count == 0 {
                ready.insert(*target);
            }
        }
    }
    if order.len() == blocks.len() {
        return order;
    }

    // A topological prefix omits both the cyclic remainder and its exits.
    // Rebuild from entry rather than treating omitted blocks as validated.
    order.clear();
    let mut visited = BTreeSet::new();
    let mut pending = vec![entry];
    while let Some(block) = pending.pop() {
        if visited.insert(block) {
            order.push(block);
            pending.extend(successors[&block].iter().rev().copied());
        }
    }
    order
}
