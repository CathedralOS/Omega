/*
Closed analysis-product catalog (`AnalysisProduct`), lazy per-revision
computation dispatch (`compute_analysis`), and content-validated unit revision
binding (`AnalysisRevision`) for the post-Terminal abstract optimizer. Each
analysis product computes deterministically from the immutable
`PsiOptimizationUnit` revision alone and cannot outlive the unit borrow it
describes.
*/

pub(crate) mod catalog {
    use optimization_core::AnalysisKind;
    use optimization_unit::PsiOptimizationUnit;

    use super::{
        control_flow::{
            CallGraphAnalysis, ControlFlowAnalysis, DominatorAnalysis, LoopAnalysis,
            StronglyConnectedComponentAnalysis, block_components, call_graph, control_flow,
            dominators, loops,
        },
        semantic::{
            EffectSummaryAnalysis, ExecutableEdgeAnalysis, OwnershipFrontierAnalysis,
            PlaceAccessSummaryAnalysis, PlaceAliasesAnalysis, ScalarConstantAnalysis,
            UseDefinitionAnalysis, ValueLivenessAnalysis, ValueRangeAnalysis, effect_summaries,
            executable_edges, ownership_frontiers, place_access_summaries, place_aliases,
            scalar_constants, use_definitions, value_liveness, value_ranges,
        },
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum AnalysisProduct {
        ControlFlowGraph(ControlFlowAnalysis),
        Dominators(DominatorAnalysis),
        PostDominators(DominatorAnalysis),
        LoopForest(LoopAnalysis),
        StronglyConnectedComponents(StronglyConnectedComponentAnalysis),
        CallGraph(CallGraphAnalysis),
        PlaceAliases(PlaceAliasesAnalysis),
        UseDefinition(UseDefinitionAnalysis),
        ExecutableEdges(ExecutableEdgeAnalysis),
        ScalarConstants(ScalarConstantAnalysis),
        ValueRanges(ValueRangeAnalysis),
        EffectSummaries(EffectSummaryAnalysis),
        OwnershipFrontiers(OwnershipFrontierAnalysis),
        ValueLiveness(ValueLivenessAnalysis),
        PlaceAccessSummaries(PlaceAccessSummaryAnalysis),
    }

    impl AnalysisProduct {}

    pub(crate) fn compute_analysis(
        unit: &PsiOptimizationUnit,
        kind: AnalysisKind,
    ) -> Result<Option<AnalysisProduct>, &'static str> {
        Ok(match kind {
            AnalysisKind::ControlFlowGraph => {
                Some(AnalysisProduct::ControlFlowGraph(control_flow(unit)))
            }
            AnalysisKind::Dominators => Some(AnalysisProduct::Dominators(dominators(unit, false))),
            AnalysisKind::PostDominators => {
                Some(AnalysisProduct::PostDominators(dominators(unit, true)))
            }
            AnalysisKind::StronglyConnectedComponents => Some(
                AnalysisProduct::StronglyConnectedComponents(block_components(unit)),
            ),
            AnalysisKind::LoopForest => Some(AnalysisProduct::LoopForest(loops(unit))),
            AnalysisKind::CallGraph => Some(AnalysisProduct::CallGraph(call_graph(unit)?)),
            AnalysisKind::PlaceAliases => Some(AnalysisProduct::PlaceAliases(place_aliases(unit))),
            AnalysisKind::UseDefinition => {
                Some(AnalysisProduct::UseDefinition(use_definitions(unit)))
            }
            AnalysisKind::ScalarConstants => {
                Some(AnalysisProduct::ScalarConstants(scalar_constants(unit)))
            }
            AnalysisKind::ExecutableEdges => {
                Some(AnalysisProduct::ExecutableEdges(executable_edges(unit)))
            }
            AnalysisKind::ValueRanges => Some(AnalysisProduct::ValueRanges(value_ranges(unit))),
            AnalysisKind::EffectSummaries => {
                Some(AnalysisProduct::EffectSummaries(effect_summaries(unit)?))
            }
            AnalysisKind::OwnershipFrontiers => Some(AnalysisProduct::OwnershipFrontiers(
                ownership_frontiers(unit),
            )),
            AnalysisKind::ValueLiveness => {
                Some(AnalysisProduct::ValueLiveness(value_liveness(unit)))
            }
            AnalysisKind::PlaceAccessSummaries => Some(AnalysisProduct::PlaceAccessSummaries(
                place_access_summaries(unit),
            )),
            _ => None,
        })
    }
}
pub(crate) use crate::control_flow_analyses as control_flow;
pub(crate) mod revision {
    use optimization_core::{AnalysisKind, AnalysisSet};
    use optimization_unit::PsiOptimizationUnit;

    use super::control_flow::{AnalysisRootError, validate_unit_identity};
    use super::{AnalysisProduct, compute_analysis};
    use crate::RuleAnalysisView;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum AnalysisError {
        Root(AnalysisRootError),
        UnsupportedAnalysis(AnalysisKind),
        CallableProvenance {
            analysis: AnalysisKind,
            reason: &'static str,
        },
    }

    failed_display!(AnalysisError, "Psi optimizer analysis failure");

    #[derive(Debug)]
    pub struct AnalysisRevision<'a> {
        unit: &'a PsiOptimizationUnit,
        products: [Option<AnalysisProduct>; AnalysisKind::ALL.len()],
    }

    impl<'a> AnalysisRevision<'a> {
        pub(crate) fn bind(unit: &'a PsiOptimizationUnit) -> Result<Self, AnalysisError> {
            validate_unit_identity(unit).map_err(AnalysisError::Root)?;
            Ok(Self {
                unit,
                products: std::array::from_fn(|_| None),
            })
        }

        pub(crate) fn require_all(
            &mut self,
            requested: AnalysisSet,
        ) -> Result<RuleAnalysisView<'_>, AnalysisError> {
            for kind in requested.iter() {
                let slot = &mut self.products[analysis_index(kind)];
                if slot.is_none() {
                    *slot = Some(
                        compute_analysis(self.unit, kind)
                            .map_err(|reason| AnalysisError::CallableProvenance {
                                analysis: kind,
                                reason,
                            })?
                            .ok_or(AnalysisError::UnsupportedAnalysis(kind))?,
                    );
                }
            }
            Ok(RuleAnalysisView::new(&self.products, requested))
        }
    }

    pub(crate) const fn analysis_index(kind: AnalysisKind) -> usize {
        kind as usize - 1
    }
}
pub(crate) use crate::semantic_analyses as semantic;

pub(crate) use catalog::{AnalysisProduct, compute_analysis};
pub(crate) use control_flow::CallGraphAnalysis;
pub(crate) use control_flow::block_dominates;
pub(crate) use control_flow::{
    CountdownInvariantConstantAnalysisError, CountdownInvariantConstantPlacementAnalysisError,
    CountedLoopAnalysisError, ValidatedCountdownInvariantConstantAnalysis,
    ValidatedCountdownInvariantConstantPlacementAnalysis, ValidatedCountedLoopAnalysis,
};
pub(crate) use control_flow::{
    analyze_countdown_invariant_constant_placement, analyze_countdown_invariant_constants,
    analyze_counted_loops,
};
#[cfg(any(test, feature = "test-support"))]
pub use optimization_unit::{
    ValueRangeFact, ValueRangeScope, ValueRangeSupport, value_range_fact_identity,
};
pub use revision::AnalysisRevision;
pub(crate) use revision::{AnalysisError, analysis_index};
pub(crate) use semantic::{
    EffectClass, EffectKnowledge, EffectSummaryAnalysis, OwnershipFrontierAnalysis,
    ScalarConstantAnalysis, UseDefinitionAnalysis, ValueRangeAnalysis,
};
