mod control_flow;
mod manager;
mod semantic;

pub use control_flow::{
    BlockControlFlow, CallGraphAnalysis, ControlFlowAnalysis, DominatorAnalysis, ExitKind,
    FunctionControlFlow, LoopAnalysis, LoopRegion, StronglyConnectedComponentAnalysis,
    analysis_dependencies, compute_analysis,
};
pub use manager::{AnalysisManager, AnalysisManagerError, AnalysisRevisionCommit};
pub use semantic::{
    EffectClass, EffectKnowledge, EffectSummaryAnalysis, ExecutableEdgeAnalysis,
    ExecutableEdgeFact, ExecutableEdgeKnowledge, FunctionEffectSummary, NodeEffectSummary,
    NodeLiveness, ScalarConstant, ScalarConstantAnalysis, ScalarConstantFact,
    ScalarConstantSupport, UseDefinitionAnalysis, ValueFactRegion, ValueLivenessAnalysis,
    ValueLivenessBlock, ValueRangeAnalysis, ValueRangeFact,
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
    ValueLiveness(ValueLivenessAnalysis),
}

impl AnalysisProduct {
    pub const fn kind(&self) -> omega_optimization_core::AnalysisKind {
        use omega_optimization_core::AnalysisKind;
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
            Self::ValueLiveness(_) => AnalysisKind::ValueLiveness,
        }
    }
}
