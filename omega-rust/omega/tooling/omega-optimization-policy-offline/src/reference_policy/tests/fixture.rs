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

use crate::{
    OfflinePolicySplit, ValidatedOfflinePolicyCorpus, admit_external_decision_logs,
    split_for_source,
};

pub(super) fn corpus() -> ValidatedOfflinePolicyCorpus {
    corpus_with_prefix(b"reference")
}

pub(super) fn corpus_with_prefix(prefix: &[u8]) -> ValidatedOfflinePolicyCorpus {
    let training = source_for_split(prefix, OfflinePolicySplit::Training);
    let evaluation = source_for_split(prefix, OfflinePolicySplit::Evaluation);
    let regression = source_for_split(prefix, OfflinePolicySplit::Regression);
    admit_external_decision_logs([
        log(
            training,
            [
                sole_point(prefix, b"train-negative-three", -3, true),
                sole_point(prefix, b"train-negative-one", -1, true),
                sole_point(prefix, b"train-positive-two", 2, false),
            ],
        ),
        log(
            evaluation,
            [
                tied_point(prefix, b"evaluation-tie", -2),
                sole_point(prefix, b"evaluation-boundary", 0, false),
                sole_point(prefix, b"evaluation-negative", -1, true),
            ],
        ),
        log(
            regression,
            [
                sole_point(prefix, b"regression-false-choose", -4, false),
                sole_point(prefix, b"regression-false-skip", 1, true),
                mismatched_choice_point(prefix, b"regression-wrong-choice"),
            ],
        ),
    ])
    .unwrap()
}

pub(super) fn tie_training_corpus() -> ValidatedOfflinePolicyCorpus {
    let source = source_for_split(b"tie-training", OfflinePolicySplit::Training);
    admit_external_decision_logs([log(
        source,
        [
            sole_point(b"tie", b"skip", -1, false),
            sole_point(b"tie", b"choose", -1, true),
        ],
    )])
    .unwrap()
}

pub(super) fn corpus_without(split: OfflinePolicySplit) -> ValidatedOfflinePolicyCorpus {
    let retained = match split {
        OfflinePolicySplit::Training => OfflinePolicySplit::Evaluation,
        OfflinePolicySplit::Evaluation | OfflinePolicySplit::Regression => {
            OfflinePolicySplit::Training
        }
    };
    let source = source_for_split(b"empty-split", retained);
    admit_external_decision_logs([log(source, [sole_point(b"empty", b"retained", -1, true)])])
        .unwrap()
}

pub(super) fn i128_aggregate_corpus() -> ValidatedOfflinePolicyCorpus {
    let training = source_for_split(b"i128", OfflinePolicySplit::Training);
    let evaluation = source_for_split(b"i128", OfflinePolicySplit::Evaluation);
    admit_external_decision_logs([
        log(training, [sole_point(b"i128", b"training", i64::MAX, true)]),
        log(
            evaluation,
            [
                sole_point(b"i128", b"first", i64::MIN, true),
                sole_point(b"i128", b"second", i64::MIN, true),
            ],
        ),
    ])
    .unwrap()
}

pub(super) fn canonical_tie_candidate(prefix: &[u8], name: &[u8]) -> OptimizationCandidateIdentity {
    let first = candidate(prefix, name, b"-a");
    let second = candidate(prefix, name, b"-b");
    first.min(second)
}

fn sole_point(prefix: &[u8], name: &[u8], cost: i64, choose: bool) -> ExternalDecisionPoint {
    let candidate = candidate(prefix, name, b"-candidate");
    point(
        prefix,
        name,
        [feature(candidate, cost)],
        if choose {
            ExternalDecisionAction::Choose(candidate)
        } else {
            ExternalDecisionAction::Skip(OptimizationReasonCode::NotProfitable)
        },
    )
}

fn tied_point(prefix: &[u8], name: &[u8], cost: i64) -> ExternalDecisionPoint {
    let first = candidate(prefix, name, b"-a");
    let second = candidate(prefix, name, b"-b");
    point(
        prefix,
        name,
        [feature(first, cost), feature(second, cost)],
        ExternalDecisionAction::Choose(first.min(second)),
    )
}

fn mismatched_choice_point(prefix: &[u8], name: &[u8]) -> ExternalDecisionPoint {
    let cheaper = candidate(prefix, name, b"-cheaper");
    let recorded = candidate(prefix, name, b"-recorded");
    point(
        prefix,
        name,
        [feature(cheaper, -2), feature(recorded, -1)],
        ExternalDecisionAction::Choose(recorded),
    )
}

fn point(
    prefix: &[u8],
    name: &[u8],
    candidates: impl IntoIterator<Item = ExternalCandidateFeatures>,
    action: ExternalDecisionAction,
) -> ExternalDecisionPoint {
    ExternalDecisionPoint::new(
        OptimizationUnitIdentity::from_canonical_bytes(&[prefix, name, b"-input"].concat()),
        OptimizationRuleIdentity::from_canonical_bytes(&[prefix, name, b"-rule"].concat()),
        candidates,
        action,
    )
    .unwrap()
}

fn feature(candidate: OptimizationCandidateIdentity, cost: i64) -> ExternalCandidateFeatures {
    ExternalCandidateFeatures::new(
        ValidatedCandidateSummary {
            candidate,
            predicted_cost_delta: cost,
        },
        AnalysisSet::new([AnalysisKind::ScalarConstants]),
        [],
    )
    .unwrap()
}

fn candidate(prefix: &[u8], name: &[u8], suffix: &[u8]) -> OptimizationCandidateIdentity {
    OptimizationCandidateIdentity::from_canonical_bytes(&[prefix, name, suffix].concat())
}

fn log(
    source: OptimizationUnitIdentity,
    points: impl IntoIterator<Item = ExternalDecisionPoint>,
) -> Vec<u8> {
    ExternalDecisionLog::new(context(source), points)
        .unwrap()
        .encode()
}

fn context(source: OptimizationUnitIdentity) -> ExternalDecisionContext {
    ExternalDecisionContext::new(
        external_psi_decision_schema_v2_identity(),
        source,
        OptimizationSelectionIdentity::from_bytes([1; 32]),
        OptimizationSelectionIdentity::from_bytes([2; 32]),
        psi_target_neutral_decision_target_v2_identity(),
        OptimizationRuleSetIdentity::from_canonical_bytes(b"offline-reference-rules"),
        TargetCostModelIdentity::from_canonical_bytes(b"offline-reference-costs"),
    )
}

fn source_for_split(prefix: &[u8], split: OfflinePolicySplit) -> OptimizationUnitIdentity {
    for ordinal in 0_u64..100_000 {
        let source = OptimizationUnitIdentity::from_canonical_bytes(
            &[prefix, &ordinal.to_le_bytes()].concat(),
        );
        if split_for_source(source) == split {
            return source;
        }
    }
    panic!("deterministic split search exhausted")
}
