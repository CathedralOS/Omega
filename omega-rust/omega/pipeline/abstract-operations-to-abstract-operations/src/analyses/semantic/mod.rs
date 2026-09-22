//! Optimizer module role: stage group. Target-neutral semantic analyses and their catalog-facing compute joins.
//!
//! Each leaf owns one independently cached fact family. The two immutable
//! scalar-CFG projections more than one analysis reads -- where a scalar
//! value is defined, and which edges a scalar operation leaves through --
//! live here, below their common owner.

mod effect_summaries;
mod ownership_frontiers;
mod place_aliases;
mod sparse_conditional_constants;
mod use_definitions;
mod value_liveness;
mod value_ranges;

pub use effect_summaries::{
    EffectClass, EffectKnowledge, EffectSummaryAnalysis, FunctionEffectSummary, NodeEffectSummary,
};
pub use ownership_frontiers::{OwnershipFrontierAnalysis, OwnershipFrontierAnalysisFact};
pub use place_aliases::{
    PlaceAliasClaim, PlaceAliasFunction, PlaceAliasRelation, PlaceAliasRoot, PlaceAliasesAnalysis,
    PlaceView,
};
pub use sparse_conditional_constants::{
    ExecutableEdgeAnalysis, ExecutableEdgeFact, ExecutableEdgeKnowledge, ScalarConstant,
    ScalarConstantAnalysis, ScalarConstantFact, ScalarConstantSupport, ValueFactRegion,
};
pub use use_definitions::UseDefinitionAnalysis;
pub use value_liveness::{NodeLiveness, ValueLivenessAnalysis, ValueLivenessBlock};
pub use value_ranges::ValueRangeAnalysis;

pub(super) use effect_summaries::effect_summaries;
pub(super) use ownership_frontiers::ownership_frontiers;
pub(super) use place_aliases::place_aliases;
pub(super) use sparse_conditional_constants::{executable_edges, scalar_constants};
pub(super) use use_definitions::use_definitions;
pub(super) use value_liveness::value_liveness;
pub(super) use value_ranges::value_ranges;

use abstract_operations::AbstractOperation as O;
use optimization_unit::{
    FuelSettlement, OptimizationEdge, PsiOptimizationFunction, PsiProvenance, ValueDefinition,
};
use semantic_vocabulary::ValueId;

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
            trivial_affine_discards,
            residual_affine_discards,
        } => vec![OptimizationEdge {
            structural_bindings: structural_bindings.clone(),
            psi_edge: *psi_edge,
            target: *target,
            bindings: bindings.clone(),
            trivial_affine_discards: trivial_affine_discards.clone(),
            residual_affine_discards: residual_affine_discards.clone(),
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
                trivial_affine_discards: successor.trivial_affine_discards.clone(),
                residual_affine_discards: Vec::new(),
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
                trivial_affine_discards: case.trivial_affine_discards.clone(),
                residual_affine_discards: Vec::new(),
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

#[cfg(test)]
mod residual_edge_tests {
    #[test]
    fn successor_projection_retains_exact_ordered_residuals() {
        use semantic_vocabulary::{BlockId, EdgeId, PlaceId, StructuralTypeId};
        use terminal_psi::{StructuralAffineDiscard, StructuralPathSegment};
        let residuals = [2, 0]
            .map(|element| StructuralAffineDiscard {
                place: PlaceId::new(81).unwrap(),
                path: vec![StructuralPathSegment::FixedIndex(element)],
                structural_type: StructuralTypeId::new(82).unwrap(),
            })
            .to_vec();
        let operation = abstract_operations::AbstractOperation::Jump {
            structural_bindings: Vec::new(),
            psi_edge: EdgeId::new(83).unwrap(),
            target: BlockId::new(84).unwrap(),
            bindings: Vec::new(),
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: residuals.clone(),
        };
        let successors = super::scalar_operation_successors(&operation);
        assert_eq!(successors.len(), 1);
        assert_eq!(successors[0].residual_affine_discards, residuals);
        assert!(successors[0].trivial_affine_discards.is_empty());
        assert_eq!(successors[0].target, BlockId::new(84).unwrap());
        assert_eq!(successors[0].psi_edge, EdgeId::new(83).unwrap());
    }
}
