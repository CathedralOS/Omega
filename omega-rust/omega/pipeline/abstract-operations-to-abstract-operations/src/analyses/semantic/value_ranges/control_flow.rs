//! Reachability and dominance regions for proof-supported facts.

use std::collections::{BTreeMap, BTreeSet};

use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, MachineId};

use crate::analyses::control_flow::DominatorAnalysis;

use super::super::shared::scalar_operation_successors;

pub(super) fn dominated_blocks(
    dominators: &DominatorAnalysis,
    machine: MachineId,
    anchor: BlockId,
) -> Vec<BlockId> {
    let mut blocks = dominators
        .functions
        .iter()
        .find(|(candidate, _)| *candidate == machine)
        .into_iter()
        .flat_map(|(_, rows)| rows)
        .filter_map(|(block, values)| values.contains(&anchor).then_some(*block))
        .collect::<Vec<_>>();
    blocks.sort_unstable();
    blocks.dedup();
    blocks
}

pub(super) fn reachable_blocks(function: &PsiOptimizationFunction) -> BTreeSet<BlockId> {
    let blocks = function
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    let mut reachable = BTreeSet::new();
    let mut pending = vec![function.entry];
    while let Some(block) = pending.pop() {
        if !reachable.insert(block) {
            continue;
        }
        let Some(block) = blocks.get(&block) else {
            continue;
        };
        let Some(terminal) = block.nodes.last() else {
            continue;
        };
        pending.extend(
            scalar_operation_successors(&terminal.operation)
                .into_iter()
                .map(|edge| edge.target),
        );
    }
    reachable
}
