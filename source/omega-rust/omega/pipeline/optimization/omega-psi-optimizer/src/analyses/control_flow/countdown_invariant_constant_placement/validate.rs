//! Optimizer module role: acceptance leaf. Independent replay and exact placement comparison.

use super::*;

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
    if candidate.revision != unit.identity {
        return Err(
            CountdownInvariantConstantPlacementAnalysisError::CandidateRevisionMismatch {
                candidate: candidate.revision,
                current: unit.identity,
            },
        );
    }
    let reconstructed = replay::reconstruct(unit, custody, counted, invariants)?;
    if *candidate != reconstructed {
        return Err(CountdownInvariantConstantPlacementAnalysisError::SnapshotMismatch);
    }
    Ok(ValidatedCountdownInvariantConstantPlacementAnalysis::new(
        reconstructed,
    ))
}
