//! Deterministic CFG scheduling, shared arrival facts, and normal-exit alternatives.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{BlockId, EdgeId, Proposition};
use terminal_psi::{TerminalMachine, Terminator};

use super::super::substitution::{FactIndex, SharedRoster};

pub(super) fn deterministic_block_order(
    machine: &TerminalMachine,
    ignored_backedges: &BTreeSet<EdgeId>,
) -> Vec<BlockId> {
    let mut successors = BTreeMap::<_, Vec<_>>::new();
    let mut indegree = machine
        .blocks
        .iter()
        .map(|block| (block.id, 0usize))
        .collect::<BTreeMap<_, _>>();
    for block in &machine.blocks {
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
        let targets = edges
            .into_iter()
            .filter_map(|(edge, target)| (!ignored_backedges.contains(&edge)).then_some(target))
            .collect::<Vec<_>>();
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
    incoming: &mut BTreeMap<BlockId, Vec<SharedRoster>>,
    block: BlockId,
) -> Vec<Proposition> {
    let paths = incoming
        .remove(&block)
        .expect("validated reachable block has incoming facts");
    intersect_paths(paths).expect("block has an incoming path")
}

pub(super) fn guaranteed_exit_facts(exits: Vec<SharedRoster>) -> Vec<Proposition> {
    let mut guaranteed = intersect_paths(exits.clone()).unwrap_or_default();
    if exits.len() < 2 {
        return guaranteed;
    }
    // Every normal exit establishes its own conjunction. Intersection alone
    // loses a restored range when different branches store different valid
    // values. Retain the residual alternatives as one disjunction: a proof
    // must establish its consequence from every arm, never select an arm.
    // Common facts stay outside it, bounding storage by the original exit
    // rosters without enumerating control-flow paths or multiplying cases.
    let common = FactIndex::new(&guaranteed);
    let mut alternatives = Vec::new();
    for exit in &exits {
        let residual = exit
            .to_vec()
            .into_iter()
            .filter(|fact| common.position_of(&guaranteed, fact).is_none())
            .collect::<Vec<_>>();
        if residual.is_empty() {
            return guaranteed;
        }
        let alternative = if residual.len() == 1 {
            residual.into_iter().next().unwrap()
        } else {
            Proposition::Conjunction(residual)
        };
        if !alternatives.contains(&alternative) {
            alternatives.push(alternative);
        }
    }
    if alternatives.len() == 1 {
        guaranteed.extend(alternatives);
    } else {
        guaranteed.push(Proposition::Disjunction(alternatives));
    }
    guaranteed
}

fn intersect_paths(paths: Vec<SharedRoster>) -> Option<Vec<Proposition>> {
    let mut paths = paths.iter();
    let mut guaranteed = paths.next()?.to_vec();
    for path in paths {
        // One fingerprint pass per incoming path turns each retained fact's
        // membership check into a bucket lookup instead of a roster scan;
        // `position_of` resolves full equality, so all-path membership
        // semantics match `contains` exactly.
        let index = FactIndex::new(path);
        guaranteed.retain(|fact| index.position_of(path, fact).is_some());
    }
    Some(guaranteed)
}
