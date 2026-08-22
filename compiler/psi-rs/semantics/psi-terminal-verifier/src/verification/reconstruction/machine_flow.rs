//! Deterministic CFG scheduling and all-path fact intersection.

use std::collections::{BTreeMap, BTreeSet};

use psi_core::{BlockId, Proposition};
use psi_terminal::{TerminalMachine, Terminator};

pub(super) fn deterministic_block_order(machine: &TerminalMachine) -> Vec<BlockId> {
    let mut successors = BTreeMap::<_, Vec<_>>::new();
    let mut indegree = machine
        .blocks
        .iter()
        .map(|block| (block.id, 0usize))
        .collect::<BTreeMap<_, _>>();
    for block in &machine.blocks {
        let targets = match &block.terminator {
            Terminator::Jump { target, .. } => vec![*target],
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true.target, when_false.target],
            Terminator::Return { .. }
            | Terminator::ReturnUnit { .. }
            | Terminator::ReturnUnitPartialAffine { .. }
            | Terminator::ReturnUnitNominalAffine { .. }
            | Terminator::ReturnStructural { .. }
            | Terminator::Crash { .. } => Vec::new(),
        };
        for target in &targets {
            *indegree
                .get_mut(target)
                .expect("validated target has an indegree") += 1;
        }
        successors.insert(block.id, targets);
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(block, count)| (*count == 0).then_some(*block))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(machine.blocks.len());
    while let Some(block) = ready.pop_first() {
        order.push(block);
        for target in &successors[&block] {
            let count = indegree
                .get_mut(target)
                .expect("validated target has an indegree");
            *count -= 1;
            if *count == 0 {
                ready.insert(*target);
            }
        }
    }
    order
}

pub(super) fn take_guaranteed_incoming(
    incoming: &mut BTreeMap<BlockId, Vec<Vec<Proposition>>>,
    block: BlockId,
) -> Vec<Proposition> {
    let paths = incoming
        .remove(&block)
        .expect("validated reachable block has incoming facts");
    intersect_paths(paths).expect("block has an incoming path")
}

pub(super) fn guaranteed_exit_facts(exits: Vec<Vec<Proposition>>) -> Vec<Proposition> {
    intersect_paths(exits).unwrap_or_default()
}

fn intersect_paths(paths: Vec<Vec<Proposition>>) -> Option<Vec<Proposition>> {
    let mut paths = paths.into_iter();
    let mut guaranteed = paths.next()?;
    for path in paths {
        guaranteed.retain(|fact| path.contains(fact));
    }
    Some(guaranteed)
}
