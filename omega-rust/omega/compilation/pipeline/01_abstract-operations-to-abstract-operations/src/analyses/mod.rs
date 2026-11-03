//! Optimizer module role: stage group.
mod catalog;
mod control_flow;
mod revision;
mod semantic;

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
