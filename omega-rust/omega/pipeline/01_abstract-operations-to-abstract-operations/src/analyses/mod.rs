//! Optimizer module role: stage group.
mod catalog;
mod control_flow;
mod manager;
mod revision;
mod semantic;

pub(crate) use catalog::analysis_dependencies;
pub use catalog::{AnalysisProduct, compute_analysis};
pub use control_flow::{CallGraphAnalysis, StronglyConnectedComponentAnalysis};
pub use control_flow::{
    CountdownInvariantConstantAnalysisError, CountdownInvariantConstantPlacementAnalysisError,
    CountedLoopAnalysisError, ValidatedCountdownInvariantConstantAnalysis,
    ValidatedCountdownInvariantConstantPlacementAnalysis, ValidatedCountedLoopAnalysis,
};
#[cfg(any(test, feature = "test-support"))]
pub use control_flow::{
    CountdownInvariantConstantAnalysisSnapshot,
    CountdownInvariantConstantPlacementAnalysisSnapshot, CountdownInvariantConstantRole,
    CountdownInvariantIntegerConstant, CountedLoopAnalysisSnapshot,
    UnsignedCountdownInvariantConstantPlacements,
};
pub(crate) use control_flow::{
    analyze_countdown_invariant_constant_placement, analyze_countdown_invariant_constants,
    analyze_counted_loops,
};
#[cfg(any(test, feature = "test-support"))]
pub(crate) use control_flow::{
    validate_countdown_invariant_constant_analysis,
    validate_countdown_invariant_constant_placement_analysis, validate_counted_loop_analysis,
};
pub use manager::{AnalysisManager, AnalysisManagerError};
#[cfg(any(test, feature = "test-support"))]
pub use optimization_unit::{
    ValueRangeFact, ValueRangeScope, ValueRangeSupport, value_range_fact_identity,
};
pub use revision::AnalysisRevision;
pub use semantic::{
    EffectClass, EffectKnowledge, EffectSummaryAnalysis, OwnershipFrontierAnalysis, ScalarConstant,
    ScalarConstantAnalysis, UseDefinitionAnalysis, ValueRangeAnalysis,
};
#[cfg(test)]
pub(crate) use semantic::{
    ExecutableEdgeKnowledge, PlaceAliasRelation, PlaceView, ScalarConstantSupport,
};

#[cfg(test)]
mod tests;
