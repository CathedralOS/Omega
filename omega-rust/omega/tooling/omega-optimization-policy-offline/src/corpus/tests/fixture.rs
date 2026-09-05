use omega_optimization_core::{
    AnalysisKind, AnalysisSet, OptimizationCandidateIdentity, OptimizationReasonCode,
    OptimizationRuleIdentity, OptimizationRuleSetIdentity, OptimizationSelectionIdentity,
    OptimizationUnitIdentity, TargetCostModelIdentity,
};
use omega_optimization_core::{
    ExternalCandidateFeatures, ExternalDecisionAction, ExternalDecisionContext,
    ExternalDecisionLog, ExternalDecisionPoint, ValidatedCandidateSummary,
    external_psi_decision_schema_v2_identity, psi_target_neutral_decision_target_v2_identity,
};

pub(super) fn source(name: &[u8]) -> OptimizationUnitIdentity {
    OptimizationUnitIdentity::from_canonical_bytes(name)
}

pub(super) fn context(source: OptimizationUnitIdentity) -> ExternalDecisionContext {
    ExternalDecisionContext::new(
        external_psi_decision_schema_v2_identity(),
        source,
        OptimizationSelectionIdentity::from_bytes([1; 32]),
        OptimizationSelectionIdentity::from_bytes([2; 32]),
        psi_target_neutral_decision_target_v2_identity(),
        OptimizationRuleSetIdentity::from_canonical_bytes(b"offline-rules"),
        TargetCostModelIdentity::from_canonical_bytes(b"offline-costs"),
    )
}

pub(super) fn point(name: &[u8], action: ExternalDecisionAction) -> ExternalDecisionPoint {
    let first = feature(
        &[name, b"-first"].concat(),
        -2,
        AnalysisKind::ScalarConstants,
    );
    let second = feature(&[name, b"-second"].concat(), 1, AnalysisKind::ValueRanges);
    ExternalDecisionPoint::new(
        source(&[name, b"-input"].concat()),
        OptimizationRuleIdentity::from_canonical_bytes(&[name, b"-rule"].concat()),
        [first, second],
        action,
    )
    .unwrap()
}

pub(super) fn chosen_point(name: &[u8]) -> ExternalDecisionPoint {
    point(
        name,
        ExternalDecisionAction::Choose(OptimizationCandidateIdentity::from_canonical_bytes(
            &[name, b"-first"].concat(),
        )),
    )
}

pub(super) fn skipped_point(name: &[u8]) -> ExternalDecisionPoint {
    point(
        name,
        ExternalDecisionAction::Skip(OptimizationReasonCode::NotProfitable),
    )
}

pub(super) fn encoded_log(
    source: OptimizationUnitIdentity,
    points: impl IntoIterator<Item = ExternalDecisionPoint>,
) -> Vec<u8> {
    ExternalDecisionLog::new(context(source), points)
        .unwrap()
        .encode()
}

fn feature(name: &[u8], cost: i64, analysis: AnalysisKind) -> ExternalCandidateFeatures {
    ExternalCandidateFeatures::new(
        ValidatedCandidateSummary {
            candidate: OptimizationCandidateIdentity::from_canonical_bytes(name),
            predicted_cost_delta: cost,
        },
        AnalysisSet::new([analysis]),
        [],
    )
    .unwrap()
}
