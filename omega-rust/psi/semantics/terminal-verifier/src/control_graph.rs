//! Source-independent analysis of an already target-validated control graph.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{BlockId, EdgeId};
use terminal_psi::{TerminalMachine, Terminator};

fn successors(machine: &TerminalMachine) -> BTreeMap<BlockId, Vec<(EdgeId, BlockId)>> {
    machine
        .blocks
        .iter()
        .map(|block| {
            let edges = match &block.terminator {
                Terminator::Jump { edge, target, .. } => vec![(*edge, *target)],
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => vec![
                    (when_true.edge, when_true.target),
                    (when_false.edge, when_false.target),
                ],
                Terminator::StructuralCase { cases, .. } => cases
                    .iter()
                    .map(|successor| (successor.edge, successor.target))
                    .collect(),
                Terminator::Return { .. }
                | Terminator::ReturnUnit { .. }
                | Terminator::ReturnUnitPartialAffine { .. }
                | Terminator::ReturnUnitNominalAffine { .. }
                | Terminator::ReturnStructural { .. }
                | Terminator::Crash { .. } => Vec::new(),
            };
            (block.id, edges)
        })
        .collect()
}

/// A cyclic block definition is available only when it dominates the use in
/// the full graph, not merely the first traversal with backedges removed.
pub(crate) fn dominators(machine: &TerminalMachine) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
    let successors = successors(machine);
    let all_blocks = successors.keys().copied().collect::<BTreeSet<_>>();
    let mut predecessors = all_blocks
        .iter()
        .map(|block| (*block, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for (block, edges) in &successors {
        for (_, target) in edges {
            predecessors
                .get_mut(target)
                .expect("validated target")
                .insert(*block);
        }
    }
    let mut dominators = all_blocks
        .iter()
        .map(|block| {
            (
                *block,
                if *block == machine.entry {
                    BTreeSet::from([*block])
                } else {
                    all_blocks.clone()
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    loop {
        let mut changed = false;
        for block in all_blocks
            .iter()
            .copied()
            .filter(|block| *block != machine.entry)
        {
            let mut incoming = predecessors[&block].iter();
            let first = incoming
                .next()
                .expect("reachable non-entry block has a predecessor");
            let mut common = dominators[first].clone();
            for predecessor in incoming {
                common.retain(|candidate| dominators[predecessor].contains(candidate));
            }
            common.insert(block);
            if dominators[&block] != common {
                dominators.insert(block, common);
                changed = true;
            }
        }
        if !changed {
            return dominators;
        }
    }
}

/// Removing DFS ancestor edges makes the remaining graph acyclic. Their
/// targets must be treated as arbitrary iteration entries for proof facts;
/// deleting the edges does not prove a loop invariant or termination.
pub(crate) fn feedback_edges(machine: &TerminalMachine) -> BTreeMap<EdgeId, BlockId> {
    let successors = successors(machine);
    let mut entered = BTreeSet::from([machine.entry]);
    let mut finished = BTreeSet::new();
    let mut pending = vec![(machine.entry, 0usize)];
    let mut feedback = BTreeMap::new();
    while let Some((block, successor_position)) = pending.last_mut() {
        let Some((edge, target)) = successors[block].get(*successor_position).copied() else {
            finished.insert(*block);
            pending.pop();
            continue;
        };
        *successor_position += 1;
        if !entered.insert(target) {
            if !finished.contains(&target) {
                feedback.insert(edge, target);
            }
        } else {
            pending.push((target, 0));
        }
    }
    feedback
}
