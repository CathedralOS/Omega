//! Optimizer module role: stage group. Target-neutral semantic analyses and their catalog-facing compute joins.
//!
//! Each leaf owns one independently cached fact family. The two immutable
//! scalar-CFG projections more than one analysis reads -- where a scalar
//! value is defined, and which edges a scalar operation leaves through --
//! live here, below their common owner.

mod effect_summaries;
mod ownership_frontiers;
mod place_access_summaries;
mod place_aliases;
mod sparse_conditional_constants;
mod use_definitions;
mod value_liveness;
mod value_ranges;

pub use effect_summaries::{EffectClass, EffectKnowledge, EffectSummaryAnalysis};

pub use ownership_frontiers::OwnershipFrontierAnalysis;

pub use place_access_summaries::PlaceAccessSummaryAnalysis;

pub use place_aliases::PlaceAliasesAnalysis;
pub use sparse_conditional_constants::{
    ExecutableEdgeAnalysis, ScalarConstant, ScalarConstantAnalysis,
};

pub use use_definitions::UseDefinitionAnalysis;
pub use value_liveness::ValueLivenessAnalysis;

pub use value_ranges::ValueRangeAnalysis;

pub(super) use effect_summaries::effect_summaries;
pub(super) use ownership_frontiers::ownership_frontiers;
pub(super) use place_access_summaries::place_access_summaries;
pub(super) use place_aliases::place_aliases;
pub(super) use sparse_conditional_constants::{executable_edges, scalar_constants};
pub(super) use use_definitions::use_definitions;
pub(super) use value_liveness::value_liveness;
pub(super) use value_ranges::value_ranges;

use std::collections::{BTreeMap, BTreeSet};

use abstract_operations::AbstractOperation as O;
use optimization_unit::{
    FuelSettlement, OptimizationEdge, PsiOptimizationFunction, PsiProvenance, ValueDefinition,
};
use semantic_vocabulary::{BlockId, ValueId};

pub(super) fn scalar_value_definition(
    function: &PsiOptimizationFunction,
    value: ValueId,
) -> Option<ValueDefinition> {
    function
        .parameters
        .iter()
        .chain(function.blocks.iter().flat_map(|block| &block.parameters))
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .flat_map(|node| &node.definitions),
        )
        .copied()
        .find(|definition| definition.value == value)
}

pub(super) fn scalar_operation_successors(operation: &O) -> Vec<OptimizationEdge> {
    match operation {
        O::Jump {
            psi_edge,
            target,
            bindings,
            structural_bindings,
            cleanup_actions,
        } => vec![OptimizationEdge {
            structural_bindings: structural_bindings.clone(),
            psi_edge: *psi_edge,
            target: *target,
            bindings: bindings.clone(),
            cleanup_actions: cleanup_actions.clone(),
            provenance: vec![PsiProvenance::Edge(*psi_edge)],
            fuel: vec![FuelSettlement {
                site: PsiProvenance::Edge(*psi_edge),
                units: 1,
            }],
        }],
        O::Conditional {
            when_true,
            when_false,
            ..
        } => [when_true, when_false]
            .into_iter()
            .map(|successor| OptimizationEdge {
                structural_bindings: successor.structural_bindings.clone(),
                psi_edge: successor.psi_edge,
                target: successor.target,
                bindings: successor.bindings.clone(),
                cleanup_actions: successor.cleanup_actions.clone(),
                provenance: vec![PsiProvenance::Edge(successor.psi_edge)],
                fuel: vec![FuelSettlement {
                    site: PsiProvenance::Edge(successor.psi_edge),
                    units: 1,
                }],
            })
            .collect(),
        O::StructuralCase { cases, .. } => cases
            .iter()
            .map(|case| OptimizationEdge {
                psi_edge: case.psi_edge,
                target: case.target,
                bindings: Vec::new(),
                structural_bindings: Vec::new(),
                cleanup_actions: case.cleanup_actions.clone(),
                provenance: vec![PsiProvenance::Edge(case.psi_edge)],
                fuel: vec![FuelSettlement {
                    site: PsiProvenance::Edge(case.psi_edge),
                    units: 1,
                }],
            })
            .collect(),
        _ => Vec::new(),
    }
}

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
                    .map(|node| scalar_operation_successors(&node.operation))
                    .unwrap_or_default()
                    .into_iter()
                    .map(|edge| edge.target)
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut reachable = BTreeSet::new();
    let mut pending = vec![function.entry];
    while let Some(block) = pending.pop() {
        if reachable.insert(block) {
            pending.extend(successors.get(&block).into_iter().flatten().copied());
        }
    }
    reachable
}
