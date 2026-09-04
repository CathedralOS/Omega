//! Optimizer module role: acceptance leaf. Independent replay and exact candidate comparison.

use super::*;

pub(super) fn accept(
    unit: &PsiOptimizationUnit,
    custody: &ValidatedOptimizerCycleComponents,
    candidate: &CountedLoopAnalysisSnapshot,
) -> Result<ValidatedCountedLoopAnalysis, CountedLoopAnalysisError> {
    let reconstructed = replay::reconstruct(unit, custody)?;
    if *candidate != reconstructed {
        return Err(CountedLoopAnalysisError::SnapshotMismatch);
    }
    Ok(ValidatedCountedLoopAnalysis::new(reconstructed))
}
