//! Optimizer module role: stage group.
mod catalog;
mod control_flow;
mod manager;
mod semantic;

pub use catalog::{AnalysisProduct, analysis_dependencies, compute_analysis};
pub use control_flow::{
    BlockControlFlow, CallGraphAnalysis, ControlFlowAnalysis,
    CountdownInvariantConstantAnalysisError, CountdownInvariantConstantAnalysisSnapshot,
    CountdownInvariantConstantConsumer, CountdownInvariantConstantDestination,
    CountdownInvariantConstantPlacement, CountdownInvariantConstantPlacementAnalysisError,
    CountdownInvariantConstantPlacementAnalysisSnapshot, CountdownInvariantConstantRole,
    CountdownInvariantIntegerConstant, CountedLoopAnalysisError, CountedLoopAnalysisSnapshot,
    DominatorAnalysis, ExactUnsignedTripCount, ExitKind, FunctionControlFlow, LoopAnalysis,
    LoopRegion, StronglyConnectedComponentAnalysis, UnsignedCountdownInvariantConstantPlacements,
    UnsignedCountdownInvariantConstants, UnsignedCountdownLoopSummary,
    ValidatedCountdownInvariantConstantAnalysis,
    ValidatedCountdownInvariantConstantPlacementAnalysis, ValidatedCountedLoopAnalysis,
};
pub(crate) use control_flow::{
    analyze_countdown_invariant_constant_placement, analyze_countdown_invariant_constants,
    analyze_counted_loops, validate_countdown_invariant_constant_analysis,
    validate_countdown_invariant_constant_placement_analysis, validate_counted_loop_analysis,
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
