//! Optimizer module role: stage group. Target-neutral semantic analyses and their catalog-facing compute joins.
//!
//! Each leaf owns one independently cached fact family. The two immutable
//! scalar-CFG projections more than one analysis reads -- where a scalar
//! value is defined, and which edges a scalar operation leaves through --
//! are stated once by their owner: `optimization_unit::scalar_value_definition`
//! and each node's construction-built `successors` roster. Analyses read
//! both; nothing here restates the edge construction.

mod effect_summaries;
mod ownership_frontiers;
mod place_access_summaries;
mod place_aliases;
mod sparse_conditional_constants;
mod use_definitions;
mod value_liveness;
mod value_ranges;

pub(crate) use effect_summaries::{EffectClass, EffectKnowledge, EffectSummaryAnalysis};

pub(crate) use ownership_frontiers::OwnershipFrontierAnalysis;

pub(crate) use place_access_summaries::PlaceAccessSummaryAnalysis;

pub(crate) use place_aliases::PlaceAliasesAnalysis;
pub(crate) use sparse_conditional_constants::{ExecutableEdgeAnalysis, ScalarConstantAnalysis};

pub(crate) use use_definitions::UseDefinitionAnalysis;
pub(crate) use value_liveness::ValueLivenessAnalysis;

pub(crate) use value_ranges::ValueRangeAnalysis;

pub(super) use effect_summaries::effect_summaries;
pub(super) use ownership_frontiers::ownership_frontiers;
pub(super) use place_access_summaries::place_access_summaries;
pub(super) use place_aliases::place_aliases;
pub(super) use sparse_conditional_constants::{executable_edges, scalar_constants};
pub(super) use use_definitions::use_definitions;
pub(super) use value_liveness::value_liveness;
pub(super) use value_ranges::value_ranges;

use std::collections::{BTreeMap, BTreeSet};

use optimization_unit::{PsiOptimizationFunction, scalar_value_definition};
use semantic_vocabulary::BlockId;

/// Blocks reachable from the entry along carried scalar-successor edges.
/// Every semantic analysis shares this reachability floor: an operation in
/// an unreachable block cannot execute, so its effects and accesses must
/// not pollute the function summary.
pub(super) fn semantically_reachable_blocks(
    function: &PsiOptimizationFunction,
) -> BTreeSet<BlockId> {
    let successors = function
        .blocks
        .iter()
        .map(|block| {
            (
                block.id,
                block
                    .nodes
                    .last()
                    .map(|node| node.successors.iter().map(|edge| edge.target).collect())
                    .unwrap_or_default(),
            )
        })
        .collect::<BTreeMap<BlockId, Vec<BlockId>>>();
    let mut reachable = BTreeSet::new();
    let mut pending = vec![function.entry];
    while let Some(block) = pending.pop() {
        if reachable.insert(block) {
            pending.extend(successors.get(&block).into_iter().flatten().copied());
        }
    }
    reachable
}
