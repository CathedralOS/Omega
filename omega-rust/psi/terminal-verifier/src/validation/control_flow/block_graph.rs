//! The successor graph of one machine's blocks and the order the checks
//! walk them in.

use crate::validation::{
    BTreeMap, BTreeSet, BlockId, ModuleError, TerminalMachine, TerminalModule, Terminator,
};

/// The block order the checks walk: successors and predecessors from each
/// terminator, every block reachable from the entry, and a topological
/// order; a cyclic graph is allowed only for an unranked-cycle machine.
pub(super) fn block_order(
    module: &TerminalModule,
    machine: &TerminalMachine,
    blocks: &BTreeMap<BlockId, &terminal_psi::Block>,
) -> Result<Vec<BlockId>, ModuleError> {
    let mut successors = BTreeMap::<BlockId, Vec<BlockId>>::new();
    let mut predecessors = blocks
        .keys()
        .map(|block| (*block, Vec::<BlockId>::new()))
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
        for (_, target) in &targets {
            if !blocks.contains_key(target) {
                return Err(ModuleError::UnknownTargetBlock(*target));
            }
        }
        for (_, target) in &targets {
            predecessors
                .get_mut(target)
                .expect("known target has a predecessor row")
                .push(block.id);
        }
        successors.insert(
            block.id,
            targets.iter().map(|(_, target)| *target).collect(),
        );
    }

    let mut reachable = BTreeSet::new();
    let mut pending = vec![machine.entry];
    while let Some(block) = pending.pop() {
        if reachable.insert(block) {
            pending.extend(
                successors
                    .get(&block)
                    .expect("every block has successors")
                    .iter()
                    .copied(),
            );
        }
    }
    if reachable.len() != blocks.len() {
        let block = blocks
            .keys()
            .find(|block| !reachable.contains(block))
            .copied()
            .expect("different set lengths guarantee an unreachable block");
        return Err(ModuleError::UnreachableBlock(block));
    }

    let mut indegree = predecessors
        .iter()
        .map(|(block, incoming)| (*block, incoming.len()))
        .collect::<BTreeMap<_, _>>();
    let mut ready = indegree
        .iter()
        .filter_map(|(block, count)| (*count == 0).then_some(*block))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(blocks.len());
    while let Some(block) = ready.pop_first() {
        order.push(block);
        for target in successors.get(&block).expect("every block has successors") {
            let count = indegree
                .get_mut(target)
                .expect("known target has an indegree");
            *count -= 1;
            if *count == 0 {
                ready.insert(*target);
            }
        }
    }
    let cyclic = order.len() != blocks.len();
    if cyclic {
        if !super::unranked_cycles::eligible(module, machine) {
            let block = indegree
                .iter()
                .find_map(|(block, count)| (*count != 0).then_some(*block))
                .expect("a cyclic graph leaves positive indegree");
            return Err(ModuleError::ControlCycle(block));
        }
        // Every target and reachable block has been checked before the shared
        // full-graph analysis. Validation below is independent of visit order.
        order = blocks.keys().copied().collect();
    }

    Ok(order)
}
