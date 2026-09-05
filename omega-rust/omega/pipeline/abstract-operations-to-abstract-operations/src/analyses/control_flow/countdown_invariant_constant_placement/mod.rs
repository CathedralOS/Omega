//! Optimizer module role: executable entrance. Exact countdown constant-placement coordination.

use abstract_operations::AbstractOperation as O;
use optimization_core::OptimizationUnitIdentity;
use optimization_unit::{
    NodeLocation, OptimizationNode, PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance,
    ValueDefinitionSite, ValueUse, recompute_psi_optimization_unit_identity,
};
use optimization_validation::{
    CycleComponentEdge, CycleComponentId, OptimizerUnsignedCountdownRankingCertificate,
    ValidatedOptimizerCycleComponents,
};
use semantic_vocabulary::{BlockId, MachineId, OperationId, ScalarType, ValueId};

use super::{
    CountdownInvariantConstantAnalysisError, CountdownInvariantConstantRole,
    CountdownInvariantIntegerConstant, CountedLoopAnalysisError, UnsignedCountdownLoopSummary,
    ValidatedCountdownInvariantConstantAnalysis, ValidatedCountedLoopAnalysis,
};

mod compute;
mod model;
mod replay;
mod validate;

pub use model::{
    CountdownInvariantConstantConsumer, CountdownInvariantConstantDestination,
    CountdownInvariantConstantPlacement, CountdownInvariantConstantPlacementAnalysisError,
    CountdownInvariantConstantPlacementAnalysisSnapshot,
    UnsignedCountdownInvariantConstantPlacements,
    ValidatedCountdownInvariantConstantPlacementAnalysis,
};

pub(crate) fn analyze_countdown_invariant_constant_placement(
    unit: &PsiOptimizationUnit,
    custody: &ValidatedOptimizerCycleComponents,
    counted: &ValidatedCountedLoopAnalysis,
    invariants: &ValidatedCountdownInvariantConstantAnalysis,
) -> Result<
    ValidatedCountdownInvariantConstantPlacementAnalysis,
    CountdownInvariantConstantPlacementAnalysisError,
> {
    let candidate = compute::propose(unit, custody, counted, invariants)?;
    validate::accept(unit, custody, counted, invariants, &candidate)
}

pub(crate) fn validate_countdown_invariant_constant_placement_analysis(
    unit: &PsiOptimizationUnit,
    custody: &ValidatedOptimizerCycleComponents,
    counted: &ValidatedCountedLoopAnalysis,
    invariants: &ValidatedCountdownInvariantConstantAnalysis,
    candidate: &CountdownInvariantConstantPlacementAnalysisSnapshot,
) -> Result<
    ValidatedCountdownInvariantConstantPlacementAnalysis,
    CountdownInvariantConstantPlacementAnalysisError,
> {
    validate::accept(unit, custody, counted, invariants, candidate)
}
