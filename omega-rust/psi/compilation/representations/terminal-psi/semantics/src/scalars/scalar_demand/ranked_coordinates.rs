//! Parameter positions and values named by retained ranking evidence.
use semantic_vocabulary::{BlockId, ValueId};
use terminal_psi::{TerminalMachine, TerminalRankedScc};
#[derive(Default)]
pub(super) struct RankedCoordinates {
    pub blocks: Vec<BlockId>,
    pub values: Vec<ValueId>,
}
pub(super) fn ranked_coordinates(machine: &TerminalMachine) -> Option<RankedCoordinates> {
    let mut coverage = RankedCoordinates::default();
    let Some(ranking) = &machine.ranked_scc else {
        return Some(coverage);
    };
    let TerminalRankedScc::Natural(components) = ranking;
    if components.is_empty() {
        return None;
    }
    for component in components {
        if component.ranks.is_empty() {
            return None;
        }
        for rank in &component.ranks {
            coverage.blocks.push(rank.block);
            coverage.values.extend(
                component
                    .rank_values(rank.values)?
                    .iter()
                    .map(|value| value.value),
            );
        }
        for edge in &component.edges {
            coverage.blocks.extend([edge.source, edge.target]);
            coverage.values.extend(
                component
                    .rank_values(edge.successor_ranks)?
                    .iter()
                    .map(|value| value.value),
            );
        }
    }
    Some(coverage)
}
