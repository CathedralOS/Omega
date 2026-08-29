use omega_optimization_core::{
    AcceptedObligationFactIdentity, AnalysisKind, AnalysisSet, CoreContractDecodeError,
    OptimizationCandidateIdentity, OptimizationFactReference, OptimizationFactReferenceDecodeError,
    OptimizationReasonCode, OptimizationRuleIdentity, OptimizationRuleSetIdentity,
    OptimizationSelectionIdentity, OptimizationUnitIdentity, OwnershipFrontierFactIdentity,
    ScalarConstantFactIdentity, TargetCostModelIdentity, ValueRangeFactIdentity,
};

use crate::{BaselinePolicy, ValidatedCandidateSummary};

use super::*;

const POINT_CANDIDATE_START: usize = 8 + 4 + 32 + 32 + 32 + 4;
const FEATURE_FIXED_WIDTH: usize = 32 + 8 + 8 + 4;

fn summary(name: &[u8], cost: i64) -> ValidatedCandidateSummary {
    ValidatedCandidateSummary {
        candidate: OptimizationCandidateIdentity::from_canonical_bytes(name),
        predicted_cost_delta: cost,
    }
}

fn scalar_fact(name: &[u8]) -> OptimizationFactReference {
    OptimizationFactReference::ScalarConstant(ScalarConstantFactIdentity::from_canonical_bytes(
        name,
    ))
}

fn obligation_fact(name: &[u8]) -> OptimizationFactReference {
    OptimizationFactReference::AcceptedObligation(
        AcceptedObligationFactIdentity::from_canonical_bytes(name),
    )
}

fn ownership_fact(name: &[u8]) -> OptimizationFactReference {
    OptimizationFactReference::OwnershipFrontier(
        OwnershipFrontierFactIdentity::from_canonical_bytes(name),
    )
}

fn range_fact(name: &[u8]) -> OptimizationFactReference {
    OptimizationFactReference::ValueRange(ValueRangeFactIdentity::from_canonical_bytes(name))
}

fn features(
    name: &[u8],
    cost: i64,
    analyses: AnalysisSet,
    facts: impl IntoIterator<Item = OptimizationFactReference>,
) -> ExternalCandidateFeatures {
    ExternalCandidateFeatures::new(summary(name, cost), analyses, facts).unwrap()
}

fn context() -> ExternalDecisionContext {
    ExternalDecisionContext::new(
        external_psi_decision_schema_v2_identity(),
        OptimizationUnitIdentity::from_canonical_bytes(b"source"),
        OptimizationSelectionIdentity::from_bytes([1; 32]),
        OptimizationSelectionIdentity::from_bytes([2; 32]),
        psi_target_neutral_decision_target_v2_identity(),
        OptimizationRuleSetIdentity::from_canonical_bytes(b"rules"),
        TargetCostModelIdentity::from_canonical_bytes(b"cost"),
    )
}

fn point() -> ExternalDecisionPoint {
    let first = features(
        b"first",
        -1,
        AnalysisSet::new([AnalysisKind::ScalarConstants, AnalysisKind::ValueRanges]),
        [scalar_fact(b"constant"), range_fact(b"range")],
    );
    let second = features(
        b"second",
        -2,
        AnalysisSet::new([AnalysisKind::Dominators, AnalysisKind::OwnershipFrontiers]),
        [obligation_fact(b"proof"), ownership_fact(b"frontier")],
    );
    ExternalDecisionPoint::new(
        OptimizationUnitIdentity::from_canonical_bytes(b"input"),
        OptimizationRuleIdentity::from_canonical_bytes(b"rule"),
        [first, second],
        ExternalDecisionAction::Choose(summary(b"second", -2).candidate),
    )
    .unwrap()
}

#[test]
fn point_canonicalizes_candidates_and_facts_without_changing_policy_outcome() {
    let summaries = [summary(b"slow", -1), summary(b"fast", -3)];
    let mut baseline = BaselinePolicy::default();
    let outcome = baseline.choose(
        OptimizationUnitIdentity::from_canonical_bytes(b"input"),
        summaries,
    );
    let slow = ExternalCandidateFeatures::new(
        summaries[0],
        AnalysisSet::new([AnalysisKind::EffectSummaries]),
        [range_fact(b"z"), scalar_fact(b"a")],
    )
    .unwrap();
    let fast = features(
        b"fast",
        -3,
        AnalysisSet::new([AnalysisKind::ScalarConstants]),
        [scalar_fact(b"fast-fact")],
    );
    let point = ExternalDecisionPoint::new(
        OptimizationUnitIdentity::from_canonical_bytes(b"input"),
        OptimizationRuleIdentity::from_canonical_bytes(b"rule"),
        [slow, fast],
        outcome.into(),
    )
    .unwrap();
    assert_eq!(point.action(), outcome.into());
    assert!(
        point
            .legal_candidates()
            .windows(2)
            .all(|pair| pair[0].candidate() < pair[1].candidate())
    );
    assert!(point.legal_candidates().iter().all(|candidate| {
        candidate
            .consumed_facts()
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    }));
}

#[test]
fn strict_v2_round_trip_binds_context_point_order_and_authoritative_features() {
    let first = point();
    let second_features = features(
        b"only",
        0,
        AnalysisSet::new([AnalysisKind::ControlFlowGraph]),
        [ownership_fact(b"only-frontier")],
    );
    let second = ExternalDecisionPoint::new(
        OptimizationUnitIdentity::from_canonical_bytes(b"next"),
        OptimizationRuleIdentity::from_canonical_bytes(b"other-rule"),
        [second_features],
        ExternalDecisionAction::Skip(OptimizationReasonCode::NotProfitable),
    )
    .unwrap();
    let log = ExternalDecisionLog::new(context(), [first.clone(), second.clone()]).unwrap();
    assert_eq!(ExternalDecisionLog::decode(&log.encode()), Ok(log.clone()));
    let reversed = ExternalDecisionLog::new(context(), [second, first]).unwrap();
    assert_ne!(log.identity(), reversed.identity());

    for offset in [44, 76, 108, 140, 172, 204, 236] {
        let mut corrupt = log.encode();
        corrupt[offset] ^= 1;
        assert_eq!(
            ExternalDecisionLog::decode(&corrupt),
            Err(ExternalDecisionSchemaError::LogIdentityMismatch)
        );
    }

    let base = features(
        b"feature-identity",
        -1,
        AnalysisSet::new([AnalysisKind::ScalarConstants]),
        [scalar_fact(b"one")],
    );
    let changed_analysis = features(
        b"feature-identity",
        -1,
        AnalysisSet::new([AnalysisKind::ValueRanges]),
        [scalar_fact(b"one")],
    );
    let changed_fact = features(
        b"feature-identity",
        -1,
        AnalysisSet::new([AnalysisKind::ScalarConstants]),
        [scalar_fact(b"two")],
    );
    let make = |candidate| {
        ExternalDecisionPoint::new(
            OptimizationUnitIdentity::from_canonical_bytes(b"feature-input"),
            OptimizationRuleIdentity::from_canonical_bytes(b"feature-rule"),
            [candidate],
            ExternalDecisionAction::Skip(OptimizationReasonCode::NotProfitable),
        )
        .unwrap()
    };
    let bound = make(base.clone());
    assert_ne!(bound.identity(), make(changed_analysis).identity());
    assert_ne!(make(base).identity(), make(changed_fact).identity());

    let mut changed_wire_analysis = bound.encode();
    changed_wire_analysis[POINT_CANDIDATE_START + 32 + 8] ^= 1 << 1;
    assert_eq!(
        ExternalDecisionPoint::decode(&changed_wire_analysis),
        Err(ExternalDecisionSchemaError::PointIdentityMismatch)
    );
    let mut changed_wire_fact = bound.encode();
    changed_wire_fact[POINT_CANDIDATE_START + FEATURE_FIXED_WIDTH + 1] ^= 1;
    assert_eq!(
        ExternalDecisionPoint::decode(&changed_wire_fact),
        Err(ExternalDecisionSchemaError::PointIdentityMismatch)
    );
}

#[test]
fn v1_wire_versions_are_rejected_before_payload_admission() {
    let log = ExternalDecisionLog::new(context(), [point()]).unwrap();
    let mut v1_log = log.encode();
    v1_log[8..12].copy_from_slice(&1_u32.to_le_bytes());
    assert_eq!(
        ExternalDecisionLog::decode(&v1_log),
        Err(ExternalDecisionSchemaError::UnsupportedLogVersion(1))
    );

    let mut v1_point = point().encode();
    v1_point[8..12].copy_from_slice(&1_u32.to_le_bytes());
    assert_eq!(
        ExternalDecisionPoint::decode(&v1_point),
        Err(ExternalDecisionSchemaError::UnsupportedPointVersion(1))
    );
}

#[test]
fn duplicate_and_noncanonical_candidate_or_fact_rows_reject() {
    let duplicate_fact = scalar_fact(b"duplicate");
    assert_eq!(
        ExternalCandidateFeatures::new(
            summary(b"candidate", -1),
            AnalysisSet::default(),
            [duplicate_fact, duplicate_fact],
        ),
        Err(ExternalDecisionSchemaError::DuplicateCandidateFact)
    );

    let candidate = features(
        b"candidate",
        -1,
        AnalysisSet::default(),
        [scalar_fact(b"one")],
    );
    assert_eq!(
        ExternalDecisionPoint::new(
            OptimizationUnitIdentity::from_canonical_bytes(b"input"),
            OptimizationRuleIdentity::from_canonical_bytes(b"rule"),
            [candidate.clone(), candidate.clone()],
            ExternalDecisionAction::Choose(candidate.candidate()),
        ),
        Err(ExternalDecisionSchemaError::DuplicateCandidate)
    );
    assert_eq!(
        ExternalDecisionPoint::new(
            OptimizationUnitIdentity::from_canonical_bytes(b"input"),
            OptimizationRuleIdentity::from_canonical_bytes(b"rule"),
            [candidate],
            ExternalDecisionAction::Choose(OptimizationCandidateIdentity::from_canonical_bytes(
                b"foreign"
            ),),
        ),
        Err(ExternalDecisionSchemaError::IllegalAction)
    );

    let first = features(b"a", -1, AnalysisSet::default(), []);
    let second = features(b"b", -2, AnalysisSet::default(), []);
    let encoded = ExternalDecisionPoint::new(
        OptimizationUnitIdentity::from_canonical_bytes(b"input"),
        OptimizationRuleIdentity::from_canonical_bytes(b"rule"),
        [first, second],
        ExternalDecisionAction::Skip(OptimizationReasonCode::NotProfitable),
    )
    .unwrap()
    .encode();
    let mut noncanonical_candidates = encoded;
    let first = noncanonical_candidates
        [POINT_CANDIDATE_START..POINT_CANDIDATE_START + FEATURE_FIXED_WIDTH]
        .to_vec();
    let second = noncanonical_candidates[POINT_CANDIDATE_START + FEATURE_FIXED_WIDTH
        ..POINT_CANDIDATE_START + FEATURE_FIXED_WIDTH * 2]
        .to_vec();
    noncanonical_candidates[POINT_CANDIDATE_START..POINT_CANDIDATE_START + FEATURE_FIXED_WIDTH]
        .copy_from_slice(&second);
    noncanonical_candidates[POINT_CANDIDATE_START + FEATURE_FIXED_WIDTH
        ..POINT_CANDIDATE_START + FEATURE_FIXED_WIDTH * 2]
        .copy_from_slice(&first);
    assert_eq!(
        ExternalDecisionPoint::decode(&noncanonical_candidates),
        Err(ExternalDecisionSchemaError::NonCanonicalCandidates)
    );

    let candidate = features(
        b"facts",
        -1,
        AnalysisSet::default(),
        [scalar_fact(b"a"), range_fact(b"z")],
    );
    let mut noncanonical_facts = ExternalDecisionPoint::new(
        OptimizationUnitIdentity::from_canonical_bytes(b"input"),
        OptimizationRuleIdentity::from_canonical_bytes(b"rule"),
        [candidate],
        ExternalDecisionAction::Skip(OptimizationReasonCode::NotProfitable),
    )
    .unwrap()
    .encode();
    let fact_start = POINT_CANDIDATE_START + FEATURE_FIXED_WIDTH;
    let width = OptimizationFactReference::ENCODED_LENGTH;
    let first = noncanonical_facts[fact_start..fact_start + width].to_vec();
    let second = noncanonical_facts[fact_start + width..fact_start + width * 2].to_vec();
    noncanonical_facts[fact_start..fact_start + width].copy_from_slice(&second);
    noncanonical_facts[fact_start + width..fact_start + width * 2].copy_from_slice(&first);
    assert_eq!(
        ExternalDecisionPoint::decode(&noncanonical_facts),
        Err(ExternalDecisionSchemaError::NonCanonicalCandidateFacts)
    );
}

#[test]
fn codec_rejects_unknown_feature_bits_tags_and_framing_tamper() {
    let zero_fact = features(b"zero", -1, AnalysisSet::default(), []);
    let mut unknown_analysis = ExternalDecisionPoint::new(
        OptimizationUnitIdentity::from_canonical_bytes(b"input"),
        OptimizationRuleIdentity::from_canonical_bytes(b"rule"),
        [zero_fact],
        ExternalDecisionAction::Skip(OptimizationReasonCode::NotProfitable),
    )
    .unwrap()
    .encode();
    let analysis_start = POINT_CANDIDATE_START + 32 + 8;
    unknown_analysis[analysis_start..analysis_start + 8]
        .copy_from_slice(&(1_u64 << 63).to_le_bytes());
    assert_eq!(
        ExternalDecisionPoint::decode(&unknown_analysis),
        Err(ExternalDecisionSchemaError::InvalidAnalysisSet(
            CoreContractDecodeError::UnknownAnalysisBits(1_u64 << 63)
        ))
    );

    let one_fact = features(b"one", -1, AnalysisSet::default(), [scalar_fact(b"fact")]);
    let mut unknown_fact = ExternalDecisionPoint::new(
        OptimizationUnitIdentity::from_canonical_bytes(b"input"),
        OptimizationRuleIdentity::from_canonical_bytes(b"rule"),
        [one_fact],
        ExternalDecisionAction::Skip(OptimizationReasonCode::NotProfitable),
    )
    .unwrap()
    .encode();
    unknown_fact[POINT_CANDIDATE_START + FEATURE_FIXED_WIDTH] = 255;
    assert_eq!(
        ExternalDecisionPoint::decode(&unknown_fact),
        Err(ExternalDecisionSchemaError::InvalidFactReference(
            OptimizationFactReferenceDecodeError::UnknownTag(255)
        ))
    );

    let duplicated = point();
    assert_eq!(
        ExternalDecisionLog::new(context(), [duplicated.clone(), duplicated]),
        Err(ExternalDecisionSchemaError::DuplicateDecisionPoint)
    );
    let log = ExternalDecisionLog::new(context(), [point()]).unwrap();
    let encoded = log.encode();
    assert_eq!(
        ExternalDecisionLog::decode(&encoded[..encoded.len() - 1]),
        Err(ExternalDecisionSchemaError::Truncated)
    );
    let mut trailing = encoded;
    trailing.push(0);
    assert_eq!(
        ExternalDecisionLog::decode(&trailing),
        Err(ExternalDecisionSchemaError::TrailingBytes)
    );
}
