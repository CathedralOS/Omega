//! Optimizer module role: executable entrance. Dormant external-policy execution adapter.
//!
//! [`execute`] is the sole process entrance: it requires an opaque verified
//! sandbox invocation, encodes one canonical request, applies exact I/O and
//! deadline limits, and settles either an independently matched response or
//! the caller's explicit fallback. `capability` owns the unforgeable launch
//! gate, `limits` owns bounded-I/O policy, `response` owns action-only custody,
//! and `settlement` owns fail-closed versus recorded-baseline behavior.

// The feature compiles this deliberately dormant adapter before a platform
// sandbox backend exists. Its uninhabited production capability is the gate.
#![cfg_attr(not(test), allow(dead_code))]

mod capability;
mod limits;
mod model;
mod response;
mod settlement;

pub(crate) use capability::VerifiedExternalPolicySandboxInvocation;
pub(crate) use limits::ExternalPolicyExecutionLimits;
pub(crate) use model::{
    ExternalPolicyExecutionError, ExternalPolicyExecutionFailure, ExternalPolicyExecutionOutcome,
    ExternalPolicyFallback, ExternalPolicySurfaceMismatch,
};

use bounded_process::{BoundedCaptureBudget, BoundedProcessInput, run_bounded_process};
use optimization_core::ExternalDecisionLog;

/// Execute one canonical policy exchange through a platform-verified sandbox.
///
/// No ordinary compiler path calls this function. More importantly, callers
/// cannot construct `VerifiedExternalPolicySandboxInvocation`; a future
/// platform backend must verify isolation and create that capability inside
/// this module before external policy execution can become available.
pub(crate) fn execute(
    invocation: VerifiedExternalPolicySandboxInvocation,
    recorded_baseline: &ExternalDecisionLog,
    limits: ExternalPolicyExecutionLimits,
    fallback: ExternalPolicyFallback,
) -> Result<ExternalPolicyExecutionOutcome, ExternalPolicyExecutionError> {
    if let Err(failure) = limits.validate() {
        return settlement::settle_failure(recorded_baseline, fallback, failure);
    }
    let request = recorded_baseline.encode();
    if request.len() > limits.request_bytes() {
        return settlement::settle_failure(
            recorded_baseline,
            fallback,
            ExternalPolicyExecutionFailure::RequestOverflow {
                limit: limits.request_bytes(),
                observed: request.len(),
            },
        );
    }

    let output = run_bounded_process(
        invocation.into_prepared(),
        BoundedProcessInput::Bytes(request),
        limits.capture_limits(),
        BoundedCaptureBudget::new(limits.captured_output_bytes()),
    )
    .map_err(ExternalPolicyExecutionFailure::Process)
    .and_then(|output| {
        if !output.status.success() {
            return Err(ExternalPolicyExecutionFailure::UnsuccessfulExit {
                code: output.status.code(),
                unix_signal: output.status.unix_signal(),
                stderr: output.stderr,
            });
        }
        let response = ExternalDecisionLog::decode(&output.stdout)
            .map_err(ExternalPolicyExecutionFailure::ResponseSchema)?;
        response::require_action_only_change(recorded_baseline, &response)
            .map_err(ExternalPolicyExecutionFailure::SurfaceMismatch)?;
        Ok(response)
    });

    match output {
        Ok(decisions) => Ok(ExternalPolicyExecutionOutcome::external(decisions)),
        Err(failure) => settlement::settle_failure(recorded_baseline, fallback, failure),
    }
}

#[cfg(test)]
mod tests;
