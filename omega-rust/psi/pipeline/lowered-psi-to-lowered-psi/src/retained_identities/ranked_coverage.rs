//! Coordinates a machine's retained ranking evidence names by identity.
//!
//! A `TerminalRankedScc` row covers exactly the cyclic components the
//! verifier derived for its machine: the member blocks, and the scalar values
//! the rows pin — rank values and successor ranks. Rewrites keep covered
//! block contents and named value identities exact; regions outside the
//! covered components optimize under the ordinary rules.

use semantic_vocabulary::{BlockId, ValueId};
use std::collections::BTreeSet;
use terminal_psi::{TerminalMachine, TerminalRankedScc};

/// The block-local coordinates a machine's `ranked_scc` evidence covers.
#[derive(Debug, Default)]
pub(crate) struct RankedCoverage {
    /// Member blocks of the covered cyclic components. Their parameter tables
    /// feed rank-substitution positions and their contents stay exact.
    pub blocks: BTreeSet<BlockId>,
    /// Scalar values the rows name by identity: rank values and successor
    /// ranks.
    pub values: BTreeSet<ValueId>,
    /// Scalar values used inside member blocks. Collapsing or removing one
    /// would rewrite covered contents through the use substitution, so its
    /// identity survives even though the evidence does not name it.
    pub member_uses: BTreeSet<ValueId>,
}

/// Inventory the coordinates `machine`'s retained ranking evidence covers.
/// An unranked machine has empty coverage and optimizes everywhere.
pub(crate) fn ranked_coverage(machine: &TerminalMachine) -> RankedCoverage {
    let mut coverage = RankedCoverage::default();
    let Some(ranking) = &machine.ranked_scc else {
        return coverage;
    };
    let TerminalRankedScc::Natural(components) = ranking;
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
