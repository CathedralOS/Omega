//! Closed analysis-product catalog and computation dispatch. Each product
//! computes from the unit alone.

use optimization_core::AnalysisKind;
use optimization_unit::PsiOptimizationUnit;

use super::{
    control_flow::{
        CallGraphAnalysis, ControlFlowAnalysis, DominatorAnalysis, LoopAnalysis,
        StronglyConnectedComponentAnalysis, block_components, call_graph, control_flow, dominators,
        loops,
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
) -> Option<AnalysisProduct> {
    match kind {
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
        AnalysisKind::CallGraph => Some(AnalysisProduct::CallGraph(call_graph(unit))),
        AnalysisKind::PlaceAliases => Some(AnalysisProduct::PlaceAliases(place_aliases(unit))),
        AnalysisKind::UseDefinition => Some(AnalysisProduct::UseDefinition(use_definitions(unit))),
        AnalysisKind::ScalarConstants => {
            Some(AnalysisProduct::ScalarConstants(scalar_constants(unit)))
        }
        AnalysisKind::ExecutableEdges => {
            Some(AnalysisProduct::ExecutableEdges(executable_edges(unit)))
        }
        AnalysisKind::ValueRanges => Some(AnalysisProduct::ValueRanges(value_ranges(unit))),
        AnalysisKind::EffectSummaries => {
            Some(AnalysisProduct::EffectSummaries(effect_summaries(unit)))
        }
        AnalysisKind::OwnershipFrontiers => Some(AnalysisProduct::OwnershipFrontiers(
            ownership_frontiers(unit),
        )),
        AnalysisKind::ValueLiveness => Some(AnalysisProduct::ValueLiveness(value_liveness(unit))),
        AnalysisKind::PlaceAccessSummaries => Some(AnalysisProduct::PlaceAccessSummaries(
            place_access_summaries(unit),
        )),
        _ => None,
    }
}
