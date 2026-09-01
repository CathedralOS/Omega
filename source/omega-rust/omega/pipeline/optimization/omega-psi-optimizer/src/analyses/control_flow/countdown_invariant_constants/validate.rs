//! Optimizer module role: acceptance leaf. Independent replay and exact candidate comparison.

use super::*;

pub(super) fn accept(
    unit: &PsiOptimizationUnit,
    custody: &ValidatedOptimizerCycleComponents,
    counted: &ValidatedCountedLoopAnalysis,
    candidate: &CountdownInvariantConstantAnalysisSnapshot,
) -> Result<ValidatedCountdownInvariantConstantAnalysis, CountdownInvariantConstantAnalysisError> {
    let reconstructed = replay::reconstruct(unit, custody, counted)?;
    if *candidate != reconstructed {
        return Err(CountdownInvariantConstantAnalysisError::SnapshotMismatch);
    }
    Ok(ValidatedCountdownInvariantConstantAnalysis::new(
        reconstructed,
    ))
}
