// Golden digest tests pinning the exact canonical identity row bytes of
// `OptimizationDecisionRecord`, `BaselineDecisionLog`, `ExternalDecisionPoint`,
// and `ExternalDecisionLog`.

use crate::optimization_core::manifest::tests::fact;
use crate::{
    AnalysisKind, AnalysisSet, BaselineDecisionLogBuilder, BaselineDecisionOutcome,
    ExternalCandidateFeatures, ExternalDecisionAction, ExternalDecisionContext,
    ExternalDecisionLog, ExternalDecisionPoint, OptimizationCandidateIdentity,
    OptimizationCandidateVerdict, OptimizationDecisionRecord, OptimizationRuleIdentity,
    OptimizationRuleSetIdentity, OptimizationSelectionIdentity, OptimizationUnitIdentity,
    OptimizationValidatorIdentity, TargetCostModelIdentity, ValidatedCandidateSummary,
    external_psi_decision_schema_identity, hex, psi_target_neutral_decision_target_identity,
};

fn candidate(name: &[u8]) -> OptimizationCandidateIdentity {
    OptimizationCandidateIdentity::from_canonical_bytes(name)
}

#[test]
fn decision_record_identity_bytes_are_stable() {
    let record = OptimizationDecisionRecord::new(
        OptimizationUnitIdentity::from_canonical_bytes(b"input"),
        candidate(b"candidate"),
        OptimizationRuleIdentity::from_canonical_bytes(b"rule"),
        OptimizationCandidateVerdict::Applied,
        AnalysisSet::new([AnalysisKind::ControlFlowGraph, AnalysisKind::ValueRanges]),
        vec![fact(b"alpha"), fact(b"beta")],
        Some(OptimizationValidatorIdentity::from_canonical_bytes(
            b"validator",
        )),
    )
    .expect("valid decision record");

    assert_eq!(
        hex(&record.identity().bytes()),
        "427a2dd2608f27e6f1619b512192e25f5ff528f7b4ca81521484a76b24f3bc42"
    );
}

#[test]
fn baseline_log_identity_bytes_are_stable() {
    let mut log = BaselineDecisionLogBuilder::default();
    for (name, cost) in [
        (b"first".as_slice(), -2_i64),
        (b"second".as_slice(), -1_i64),
    ] {
        let summary = ValidatedCandidateSummary {
            candidate: candidate(name),
            predicted_cost_delta: cost,
        };
        log.record_validated_outcome(
            OptimizationUnitIdentity::from_canonical_bytes(b"input"),
            [summary],
            BaselineDecisionOutcome::Choose(summary.candidate),
        )
        .expect("recorded choice");
    }
    let log = log.finish();

    assert_eq!(
        hex(&log.identity.bytes()),
        "09fc9b5ed5b69006dafbaebfc74dbb374e7c0d73a064e91c7b0fc8802064f6c0"
    );
}

#[test]
fn external_decision_point_and_log_identity_bytes_are_stable() {
    let first = ExternalCandidateFeatures::new(
        ValidatedCandidateSummary {
            candidate: candidate(b"first"),
            predicted_cost_delta: -3,
        },
        AnalysisSet::new([AnalysisKind::ScalarConstants]),
        [fact(b"constant")],
    )
    .expect("valid features");
    let point = ExternalDecisionPoint::new(
        OptimizationUnitIdentity::from_canonical_bytes(b"input"),
        OptimizationRuleIdentity::from_canonical_bytes(b"rule"),
        [first],
        ExternalDecisionAction::Choose(candidate(b"first")),
    )
    .expect("valid point");
    let context = ExternalDecisionContext::new(
        external_psi_decision_schema_identity(),
        OptimizationUnitIdentity::from_canonical_bytes(b"source"),
        OptimizationSelectionIdentity::from_bytes([1; 32]),
        OptimizationSelectionIdentity::from_bytes([2; 32]),
        psi_target_neutral_decision_target_identity(),
        OptimizationRuleSetIdentity::from_canonical_bytes(b"rules"),
        TargetCostModelIdentity::from_canonical_bytes(b"cost"),
    );
    let log = ExternalDecisionLog::new(context, [point.clone()]).expect("valid log");

    assert_eq!(
        hex(&point.identity().bytes()),
        "5a5aa17190fef94031328daaded44a66a90a968ddba207a9fe015cffe8b075b9"
    );
    assert_eq!(
        hex(&log.identity().bytes()),
        "065bab59580c5003a9e46d3cc0cbec1bd1312d373e0ff2a8edf17ba196a8f28a"
    );
}
