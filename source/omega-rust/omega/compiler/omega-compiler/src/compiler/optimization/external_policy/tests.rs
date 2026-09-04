use std::time::Duration;

use omega_optimization_core::{
    AnalysisSet, OptimizationCandidateIdentity, OptimizationDecisionSchemaIdentity,
    OptimizationDecisionTargetIdentity, OptimizationReasonCode, OptimizationRuleIdentity,
    OptimizationRuleSetIdentity, OptimizationSelectionIdentity, OptimizationUnitIdentity,
    TargetCostModelIdentity,
};
use omega_optimization_policy::{
    ExternalCandidateFeatures, ExternalDecisionAction, ExternalDecisionContext,
    ExternalDecisionLog, ExternalDecisionPoint, ValidatedCandidateSummary,
};

use super::model::ExternalPolicyResolution;
use super::{
    ExternalPolicyExecutionError, ExternalPolicyExecutionFailure, ExternalPolicyExecutionLimits,
    ExternalPolicyFallback, ExternalPolicySurfaceMismatch, VerifiedExternalPolicySandboxInvocation,
    execute, response,
};

fn context() -> ExternalDecisionContext {
    ExternalDecisionContext::new(
        OptimizationDecisionSchemaIdentity::from_canonical_bytes(b"test external schema"),
        OptimizationUnitIdentity::from_canonical_bytes(b"test source"),
        OptimizationSelectionIdentity::from_bytes([1; 32]),
        OptimizationSelectionIdentity::from_bytes([2; 32]),
        OptimizationDecisionTargetIdentity::from_canonical_bytes(b"test target"),
        OptimizationRuleSetIdentity::from_canonical_bytes(b"test rules"),
        TargetCostModelIdentity::from_canonical_bytes(b"test costs"),
    )
}

fn candidate(name: &[u8]) -> ExternalCandidateFeatures {
    ExternalCandidateFeatures::new(
        ValidatedCandidateSummary {
            candidate: OptimizationCandidateIdentity::from_canonical_bytes(name),
            predicted_cost_delta: -1,
        },
        AnalysisSet::default(),
        [],
    )
    .expect("canonical test candidate")
}

fn log(candidate_name: &[u8], action: ExternalDecisionAction) -> ExternalDecisionLog {
    custom_log(
        context(),
        b"test input",
        b"test rule",
        candidate_name,
        action,
    )
}

fn custom_log(
    context: ExternalDecisionContext,
    input_name: &[u8],
    rule_name: &[u8],
    candidate_name: &[u8],
    action: ExternalDecisionAction,
) -> ExternalDecisionLog {
    let point = ExternalDecisionPoint::new(
        OptimizationUnitIdentity::from_canonical_bytes(input_name),
        OptimizationRuleIdentity::from_canonical_bytes(rule_name),
        [candidate(candidate_name)],
        action,
    )
    .expect("canonical test point");
    ExternalDecisionLog::new(context, [point]).expect("canonical test log")
}

fn baseline() -> ExternalDecisionLog {
    log(
        b"candidate",
        ExternalDecisionAction::Skip(OptimizationReasonCode::NotProfitable),
    )
}

#[cfg(unix)]
fn limits(request_bytes: usize, response_bytes: usize) -> ExternalPolicyExecutionLimits {
    let stderr_bytes = 128;
    ExternalPolicyExecutionLimits::new(
        request_bytes,
        response_bytes,
        stderr_bytes,
        u64::try_from(response_bytes + stderr_bytes).expect("small test capture budget"),
        Duration::from_secs(5),
        Duration::from_millis(500),
        Duration::from_millis(5),
    )
}

#[test]
fn response_custody_allows_only_a_legal_action_change() {
    let request = baseline();
    let chosen = log(
        b"candidate",
        ExternalDecisionAction::Choose(OptimizationCandidateIdentity::from_canonical_bytes(
            b"candidate",
        )),
    );
    assert_eq!(
        response::require_action_only_change(&request, &chosen),
        Ok(())
    );

    let substituted = log(
        b"substituted candidate",
        ExternalDecisionAction::Skip(OptimizationReasonCode::NotProfitable),
    );
    assert_eq!(
        response::require_action_only_change(&request, &substituted),
        Err(ExternalPolicySurfaceMismatch::CandidateSurface { ordinal: 0 })
    );

    let changed_context = ExternalDecisionContext::new(
        OptimizationDecisionSchemaIdentity::from_canonical_bytes(b"changed schema"),
        context().source(),
        context().selections(),
        context().phase_selections(),
        context().target(),
        context().rule_set(),
        context().cost_model(),
    );
    let context_substitution = custom_log(
        changed_context,
        b"test input",
        b"test rule",
        b"candidate",
        ExternalDecisionAction::Skip(OptimizationReasonCode::NotProfitable),
    );
    assert_eq!(
        response::require_action_only_change(&request, &context_substitution),
        Err(ExternalPolicySurfaceMismatch::Context)
    );

    let missing_point = ExternalDecisionLog::new(context(), []).expect("canonical empty log");
    assert_eq!(
        response::require_action_only_change(&request, &missing_point),
        Err(ExternalPolicySurfaceMismatch::PointCount)
    );

    let input_substitution = custom_log(
        context(),
        b"changed input",
        b"test rule",
        b"candidate",
        ExternalDecisionAction::Skip(OptimizationReasonCode::NotProfitable),
    );
    assert_eq!(
        response::require_action_only_change(&request, &input_substitution),
        Err(ExternalPolicySurfaceMismatch::PointInput { ordinal: 0 })
    );

    let rule_substitution = custom_log(
        context(),
        b"test input",
        b"changed rule",
        b"candidate",
        ExternalDecisionAction::Skip(OptimizationReasonCode::NotProfitable),
    );
    assert_eq!(
        response::require_action_only_change(&request, &rule_substitution),
        Err(ExternalPolicySurfaceMismatch::PointRule { ordinal: 0 })
    );
}

#[test]
fn invalid_limits_settle_only_through_the_explicit_fallback() {
    let recorded = baseline();
    let invalid = ExternalPolicyExecutionLimits::new(
        0,
        1,
        1,
        2,
        Duration::from_secs(1),
        Duration::from_millis(100),
        Duration::from_millis(1),
    );
    let command = test_command("cat");
    let failure = execute(
        command,
        &recorded,
        invalid,
        ExternalPolicyFallback::UseRecordedBaseline,
    )
    .expect("recorded baseline is the explicit fallback");
    assert_eq!(failure.decisions(), &recorded);
    assert_eq!(
        failure.resolution(),
        &ExternalPolicyResolution::RecordedBaselineFallback(
            ExternalPolicyExecutionFailure::InvalidLimits
        )
    );

    let error = execute(
        test_command("cat"),
        &recorded,
        invalid,
        ExternalPolicyFallback::FailClosed,
    );
    assert_eq!(
        error,
        Err(ExternalPolicyExecutionError(
            ExternalPolicyExecutionFailure::InvalidLimits
        ))
    );
}

#[cfg(unix)]
#[test]
fn exact_request_and_response_caps_wrap_the_bounded_exchange() {
    let recorded = baseline();
    let encoded_len = recorded.encode().len();
    let outcome = execute(
        test_command("cat"),
        &recorded,
        limits(encoded_len, encoded_len),
        ExternalPolicyFallback::FailClosed,
    )
    .expect("echoed canonical decision log is admitted");
    assert_eq!(outcome.decisions(), &recorded);
    assert_eq!(
        outcome.resolution(),
        &ExternalPolicyResolution::ExternalResponse
    );

    let request_error = execute(
        test_command("exit 99"),
        &recorded,
        limits(encoded_len - 1, encoded_len),
        ExternalPolicyFallback::FailClosed,
    );
    assert_eq!(
        request_error,
        Err(ExternalPolicyExecutionError(
            ExternalPolicyExecutionFailure::RequestOverflow {
                limit: encoded_len - 1,
                observed: encoded_len,
            }
        ))
    );

    let response_failure = execute(
        test_command("cat"),
        &recorded,
        limits(encoded_len, encoded_len - 1),
        ExternalPolicyFallback::UseRecordedBaseline,
    )
    .expect("response overflow uses the explicit recorded fallback");
    assert_eq!(response_failure.decisions(), &recorded);
    let overflow = matches!(
        response_failure.resolution(),
        ExternalPolicyResolution::RecordedBaselineFallback(
            ExternalPolicyExecutionFailure::Process(
                omega_bounded_process::BoundedProcessRunError::OutputOverflow { .. }
            )
        )
    );
    let fail_closed_macos_cleanup = cfg!(target_os = "macos")
        && matches!(
            response_failure.resolution(),
            ExternalPolicyResolution::RecordedBaselineFallback(
                ExternalPolicyExecutionFailure::Process(
                    omega_bounded_process::BoundedProcessRunError::Cleanup(_)
                )
            )
        );
    assert!(overflow || fail_closed_macos_cleanup);
}

#[cfg(unix)]
#[test]
fn malformed_output_cannot_cross_either_fallback_mode() {
    let recorded = baseline();
    let encoded_len = recorded.encode().len();
    let failure = execute(
        test_command("printf malformed"),
        &recorded,
        limits(encoded_len, encoded_len),
        ExternalPolicyFallback::FailClosed,
    );
    assert!(matches!(
        failure,
        Err(ExternalPolicyExecutionError(
            ExternalPolicyExecutionFailure::ResponseSchema(_)
        ))
    ));

    let fallback = execute(
        test_command("printf malformed"),
        &recorded,
        limits(encoded_len, encoded_len),
        ExternalPolicyFallback::UseRecordedBaseline,
    )
    .expect("malformed output selects the explicit baseline fallback");
    assert_eq!(fallback.decisions(), &recorded);
    assert!(matches!(
        fallback.resolution(),
        ExternalPolicyResolution::RecordedBaselineFallback(
            ExternalPolicyExecutionFailure::ResponseSchema(_)
        )
    ));
}

#[cfg(unix)]
fn test_command(script: &str) -> VerifiedExternalPolicySandboxInvocation {
    use std::path::Path;
    use std::process::Command;

    use omega_bounded_process::{BoundedProcessLimits, BoundedProcessPrepared};

    let shell = Path::new("/bin/sh")
        .canonicalize()
        .expect("canonical test shell");
    let mut command = Command::new(shell);
    command.arg("-c").arg(script).env_clear();
    let prepared = BoundedProcessPrepared::new(
        command,
        BoundedProcessLimits::new(
            5,
            512 * 1024 * 1024,
            16 * 1024 * 1024,
            32,
            2,
            256 * 1024 * 1024,
            384 * 1024 * 1024,
        ),
        "unsandboxed external-policy adapter test",
    )
    .expect("prepare test process");
    VerifiedExternalPolicySandboxInvocation::for_unsandboxed_test_only(prepared)
}

#[cfg(not(unix))]
fn test_command(_script: &str) -> VerifiedExternalPolicySandboxInvocation {
    use std::process::Command;

    use omega_bounded_process::{BoundedProcessLimits, BoundedProcessPrepared};

    let command = Command::new(std::env::current_exe().expect("current test executable"));
    let prepared = BoundedProcessPrepared::new(
        command,
        BoundedProcessLimits::new(
            5,
            512 * 1024 * 1024,
            16 * 1024 * 1024,
            32,
            2,
            256 * 1024 * 1024,
            384 * 1024 * 1024,
        ),
        "unsandboxed external-policy adapter test",
    )
    .expect("prepare unlaunched test process");
    VerifiedExternalPolicySandboxInvocation::for_unsandboxed_test_only(prepared)
}
