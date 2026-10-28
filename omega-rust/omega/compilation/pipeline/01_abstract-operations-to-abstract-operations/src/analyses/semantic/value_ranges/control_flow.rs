//! Reachability and dominance regions for proof-supported facts.

use semantic_vocabulary::{BlockId, MachineId};

use crate::analyses::control_flow::DominatorAnalysis;

pub(super) fn dominated_blocks(
    dominators: &DominatorAnalysis,
    machine: MachineId,
    anchor: BlockId,
) -> Vec<BlockId> {
    let mut blocks = dominators
        .function(machine)
        .into_iter()
        .flat_map(|(_, rows)| rows)
        .filter_map(|(block, values)| values.contains(&anchor).then_some(*block))
        .collect::<Vec<_>>();
    blocks.sort_unstable();
    blocks.dedup();
    blocks
}
