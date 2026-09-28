//! Model-free candidate selection, owned by the executing pass manager.

use optimization_core::{
    BaselineDecisionOutcome, OptimizationReasonCode, ValidatedCandidateSummary,
};

pub(super) fn choose_baseline(candidates: &[ValidatedCandidateSummary]) -> BaselineDecisionOutcome {
    candidates
        .iter()
        .min_by_key(|candidate| (candidate.predicted_cost_delta, candidate.candidate))
        .filter(|candidate| candidate.predicted_cost_delta < 0)
        .map_or(
            BaselineDecisionOutcome::Skip(OptimizationReasonCode::NotProfitable),
            |candidate| BaselineDecisionOutcome::Choose(candidate.candidate),
        )
}

#[cfg(test)]
mod tests {}
