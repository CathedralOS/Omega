use optimization_core::OptimizationReasonCode;
use optimization_core::{ExternalDecisionAction, ExternalDecisionPoint};

pub(super) fn predict(
    point: &ExternalDecisionPoint,
    threshold: i128,
) -> (ExternalDecisionAction, Option<i64>) {
    let selected = point
        .legal_candidates()
        .iter()
        .filter(|candidate| i128::from(candidate.predicted_cost_delta()) < threshold)
        .min_by_key(|candidate| (candidate.predicted_cost_delta(), candidate.candidate()));
    match selected {
        Some(candidate) => (
            ExternalDecisionAction::Choose(candidate.candidate()),
            Some(candidate.predicted_cost_delta()),
        ),
        None => (
            ExternalDecisionAction::Skip(OptimizationReasonCode::NotProfitable),
            None,
        ),
    }
}
