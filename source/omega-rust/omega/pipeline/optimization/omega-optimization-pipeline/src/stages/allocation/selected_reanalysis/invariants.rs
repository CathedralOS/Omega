use omega_regalloc::ValidatedAllocationLegality;

use super::model::OptimizedSelectedReanalysisError;

pub(super) fn require_no_transitions(
    legality: &ValidatedAllocationLegality,
) -> Result<(), OptimizedSelectedReanalysisError> {
    let count = legality.receipt().entry_transition_count();
    if count != 0 {
        return Err(OptimizedSelectedReanalysisError::RemainingTransitions { count });
    }
    Ok(())
}
