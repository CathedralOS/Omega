//! Optimizer module role: acceptance leaf. Independent replay and exact placement comparison.

use super::{
    CountdownInvariantConstantPlacementAnalysisError,
    CountdownInvariantConstantPlacementAnalysisSnapshot, PsiOptimizationUnit,
    ValidatedCountdownInvariantConstantAnalysis,
    ValidatedCountdownInvariantConstantPlacementAnalysis, ValidatedCountedLoopAnalysis,
    ValidatedOptimizerCycleComponents, replay, validate_analysis_root,
};
pub(super) fn accept(
    unit: &PsiOptimizationUnit,
    custody: &ValidatedOptimizerCycleComponents,
    counted: &ValidatedCountedLoopAnalysis,
    invariants: &ValidatedCountdownInvariantConstantAnalysis,
    candidate: &CountdownInvariantConstantPlacementAnalysisSnapshot,
) -> Result<
    ValidatedCountdownInvariantConstantPlacementAnalysis,
    CountdownInvariantConstantPlacementAnalysisError,
> {
    validate_analysis_root(unit, custody, &[candidate.root])
        .map_err(CountdownInvariantConstantPlacementAnalysisError::Root)?;
    let reconstructed = replay::reconstruct(unit, custody, counted, invariants)?;
    if *candidate != reconstructed {
        return Err(CountdownInvariantConstantPlacementAnalysisError::SnapshotMismatch);
    }
    Ok(ValidatedCountdownInvariantConstantPlacementAnalysis::new(
        reconstructed,
    ))
}
