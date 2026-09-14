//! Coordinates a machine's retained ranking evidence names by identity.
//!
//! A `TerminalRankedScc` row covers exactly the cyclic components the
//! verifier derived for its machine: the member blocks, and the scalar values
//! the rows pin — rank values, successor ranks, and the legacy countdown
//! row's guard and successor-argument coordinates. Rewrites keep covered
//! block contents and named value identities exact; regions outside the
//! covered components optimize under the ordinary rules.

use semantic_vocabulary::{BlockId, ValueId};
use std::collections::BTreeSet;
use terminal_psi::{TerminalMachine, TerminalRankedScc};

/// The block-local coordinates a machine's `ranked_scc` evidence covers.
#[derive(Debug, Default)]
pub(super) struct RankedCoverage {
    /// Member blocks of the covered cyclic components. Their parameter tables
    /// feed rank-substitution positions and their contents stay exact.
    pub blocks: BTreeSet<BlockId>,
    /// Scalar values the rows name by identity: rank values, successor ranks,
    /// and the countdown row's guard and successor-argument coordinates.
    pub values: BTreeSet<ValueId>,
    /// Scalar values used inside member blocks. Collapsing or removing one
    /// would rewrite covered contents through the use substitution, so its
    /// identity survives even though the evidence does not name it.
    pub member_uses: BTreeSet<ValueId>,
}

/// Inventory the coordinates `machine`'s retained ranking evidence covers.
/// An unranked machine has empty coverage and optimizes everywhere.
pub(super) fn ranked_coverage(machine: &TerminalMachine) -> RankedCoverage {
    use terminal_psi::{TerminalRankedGuard, TerminalRankedSuccessorArgument};
    let mut coverage = RankedCoverage::default();
    let Some(ranking) = &machine.ranked_scc else {
        return coverage;
    };
    match ranking {
        TerminalRankedScc::UnsignedCountdown(component) => {
            coverage.blocks.insert(component.header);
            coverage.values.insert(component.rank_parameter);
            for row in &component.covered_cyclic_edges {
                coverage.blocks.extend([row.source, row.target]);
                match row.guard {
                    TerminalRankedGuard::UnsignedParameterPositive {
                        block,
                        condition,
                        parameter,
                        ..
                    } => {
                        coverage.blocks.insert(block);
                        coverage.values.extend([condition, parameter]);
                    }
                }
                match row.successor_argument {
                    TerminalRankedSuccessorArgument::UnsignedParameterMinusOne {
                        argument,
                        source_parameter,
                        target_parameter,
                        ..
                    } => {
                        coverage
                            .values
                            .extend([argument, source_parameter, target_parameter]);
                    }
                }
            }
        }
        TerminalRankedScc::Natural(components) => {
            for component in components {
                for rank in &component.ranks {
                    coverage.blocks.insert(rank.block);
                    coverage.values.insert(rank.value);
                }
                for edge in &component.edges {
                    coverage.blocks.extend([edge.source, edge.target]);
                    coverage.values.insert(edge.successor_rank);
                }
            }
        }
    }
    // The read-side of `map_scalar_uses` is a mutating traversal: walk clones
    // and keep every visited identity.
    for block in &machine.blocks {
        if !coverage.blocks.contains(&block.id) {
            continue;
        }
        for operation in &block.operations {
            let mut kind = operation.kind.clone();
            kind.map_scalar_uses(&mut |value| {
                coverage.member_uses.insert(value);
                value
            });
        }
        let mut terminator = block.terminator.clone();
        terminator.map_scalar_uses(&mut |value| {
            coverage.member_uses.insert(value);
            value
        });
    }
    coverage
}
