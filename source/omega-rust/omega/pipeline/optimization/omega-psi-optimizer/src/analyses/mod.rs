//! Optimizer module role: stage group.
mod catalog;
mod control_flow;
mod manager;
mod semantic;

pub use catalog::{AnalysisProduct, analysis_dependencies, compute_analysis};
pub use control_flow::{
    BlockControlFlow, CallGraphAnalysis, ControlFlowAnalysis, DominatorAnalysis, ExitKind,
    FunctionControlFlow, LoopAnalysis, LoopRegion, StronglyConnectedComponentAnalysis,
};
pub use manager::{AnalysisManager, AnalysisManagerError, AnalysisRevisionCommit};
pub use omega_optimization_unit::{
    ValueRangeFact, ValueRangeRegion, ValueRangeScope, ValueRangeSupport, value_range_fact_identity,
};
pub use semantic::{
    EffectClass, EffectKnowledge, EffectSummaryAnalysis, ExecutableEdgeAnalysis,
    ExecutableEdgeFact, ExecutableEdgeKnowledge, FunctionEffectSummary, NodeEffectSummary,
    NodeLiveness, OwnershipFrontierAnalysis, OwnershipFrontierAnalysisFact, ScalarConstant,
    ScalarConstantAnalysis, ScalarConstantFact, ScalarConstantSupport, UseDefinitionAnalysis,
    ValueFactRegion, ValueLivenessAnalysis, ValueLivenessBlock, ValueRangeAnalysis,
};

#[cfg(test)]
mod tests;
