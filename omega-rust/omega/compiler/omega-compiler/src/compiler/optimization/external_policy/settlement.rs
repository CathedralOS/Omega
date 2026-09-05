use omega_optimization_core::ExternalDecisionLog;

use super::{
    ExternalPolicyExecutionError, ExternalPolicyExecutionFailure, ExternalPolicyExecutionOutcome,
    ExternalPolicyFallback,
};

pub(super) fn settle_failure(
    recorded_baseline: &ExternalDecisionLog,
    fallback: ExternalPolicyFallback,
    failure: ExternalPolicyExecutionFailure,
) -> Result<ExternalPolicyExecutionOutcome, ExternalPolicyExecutionError> {
    match fallback {
        ExternalPolicyFallback::FailClosed => Err(ExternalPolicyExecutionError(failure)),
        ExternalPolicyFallback::UseRecordedBaseline => Ok(
            ExternalPolicyExecutionOutcome::baseline(recorded_baseline.clone(), failure),
        ),
    }
}
