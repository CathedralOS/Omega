//! Closed analysis-product catalog, dependency graph, and computation dispatch.

use omega_optimization_core::{AnalysisKind, AnalysisSet};
use omega_optimization_unit::PsiOptimizationUnit;

use super::{
    control_flow::{
        CallGraphAnalysis, ControlFlowAnalysis, DominatorAnalysis, LoopAnalysis,
        StronglyConnectedComponentAnalysis, block_components, call_graph, control_flow, dominators,
        loops,
    },
    semantic::{
        EffectSummaryAnalysis, ExecutableEdgeAnalysis, OwnershipFrontierAnalysis,
        ScalarConstantAnalysis, UseDefinitionAnalysis, ValueLivenessAnalysis, ValueRangeAnalysis,
        effect_summaries, executable_edges, ownership_frontiers, scalar_constants, use_definitions,
        value_liveness, value_ranges,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalysisProduct {
    ControlFlowGraph(ControlFlowAnalysis),
    Dominators(DominatorAnalysis),
    PostDominators(DominatorAnalysis),
    LoopForest(LoopAnalysis),
    StronglyConnectedComponents(StronglyConnectedComponentAnalysis),
    CallGraph(CallGraphAnalysis),
    UseDefinition(UseDefinitionAnalysis),
    ExecutableEdges(ExecutableEdgeAnalysis),
    ScalarConstants(ScalarConstantAnalysis),
    ValueRanges(ValueRangeAnalysis),
    EffectSummaries(EffectSummaryAnalysis),
    OwnershipFrontiers(OwnershipFrontierAnalysis),
    ValueLiveness(ValueLivenessAnalysis),
}

impl AnalysisProduct {
    pub const fn kind(&self) -> AnalysisKind {
        match self {
            Self::ControlFlowGraph(_) => AnalysisKind::ControlFlowGraph,
            Self::Dominators(_) => AnalysisKind::Dominators,
            Self::PostDominators(_) => AnalysisKind::PostDominators,
            Self::LoopForest(_) => AnalysisKind::LoopForest,
            Self::StronglyConnectedComponents(_) => AnalysisKind::StronglyConnectedComponents,
            Self::CallGraph(_) => AnalysisKind::CallGraph,
            Self::UseDefinition(_) => AnalysisKind::UseDefinition,
            Self::ExecutableEdges(_) => AnalysisKind::ExecutableEdges,
            Self::ScalarConstants(_) => AnalysisKind::ScalarConstants,
            Self::ValueRanges(_) => AnalysisKind::ValueRanges,
            Self::EffectSummaries(_) => AnalysisKind::EffectSummaries,
            Self::OwnershipFrontiers(_) => AnalysisKind::OwnershipFrontiers,
            Self::ValueLiveness(_) => AnalysisKind::ValueLiveness,
        }
    }
}

pub fn analysis_dependencies(kind: AnalysisKind) -> Option<AnalysisSet> {
    match kind {
        AnalysisKind::ControlFlowGraph
        | AnalysisKind::CallGraph
        | AnalysisKind::UseDefinition
        | AnalysisKind::EffectSummaries
        | AnalysisKind::OwnershipFrontiers => Some(AnalysisSet::default()),
        AnalysisKind::Dominators
        | AnalysisKind::PostDominators
        | AnalysisKind::StronglyConnectedComponents => {
            Some(AnalysisSet::new([AnalysisKind::ControlFlowGraph]))
        }
        AnalysisKind::LoopForest => Some(AnalysisSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::Dominators,
            AnalysisKind::StronglyConnectedComponents,
        ])),
        AnalysisKind::ScalarConstants => Some(AnalysisSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::UseDefinition,
        ])),
        AnalysisKind::ExecutableEdges => Some(AnalysisSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::ScalarConstants,
        ])),
        AnalysisKind::ValueRanges => Some(AnalysisSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::Dominators,
            AnalysisKind::UseDefinition,
            AnalysisKind::ScalarConstants,
        ])),
        AnalysisKind::ValueLiveness => Some(AnalysisSet::new([
            AnalysisKind::ControlFlowGraph,
            AnalysisKind::UseDefinition,
        ])),
        _ => None,
    }
}

pub fn compute_analysis(unit: &PsiOptimizationUnit, kind: AnalysisKind) -> Option<AnalysisProduct> {
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
        _ => None,
    }
}
