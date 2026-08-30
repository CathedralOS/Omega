//! Decision-v5 validation, identity binding, and corruption rejection.

use super::*;

#[test]
fn applied_decision_requires_independent_validator_and_round_trips() {
    let rule = rule(b"rule");
    assert_eq!(
        OptimizationDecisionRecord::new(
            OptimizationUnitIdentity::from_canonical_bytes(b"input"),
            OptimizationCandidateIdentity::from_canonical_bytes(b"candidate"),
            rule,
            OptimizationCandidateVerdict::Applied,
            AnalysisSet::default(),
            Vec::new(),
            None,
        ),
        Err(InvalidOptimizationManifestRecord::AppliedWithoutValidator)
    );
    let decision = decision(rule);
    assert_eq!(
        OptimizationDecisionRecord::decode(&decision.encode()),
        Ok(decision)
    );
}

#[test]
fn value_range_fact_reference_round_trips_in_decision_v5() {
    let record = OptimizationDecisionRecord::new(
        OptimizationUnitIdentity::from_canonical_bytes(b"range-input"),
        OptimizationCandidateIdentity::from_canonical_bytes(b"range-candidate"),
        rule(b"range-rule"),
        OptimizationCandidateVerdict::Applied,
        AnalysisSet::new([AnalysisKind::ScalarConstants, AnalysisKind::ValueRanges]),
        vec![fact(b"literal"), range_fact(b"proof-range")],
        Some(OptimizationValidatorIdentity::from_canonical_bytes(
            b"range-validator",
        )),
    )
    .unwrap();
    assert_eq!(
        OptimizationDecisionRecord::decode(&record.encode()).unwrap(),
        record
    );
}

#[test]
fn decision_identity_binds_every_authoritative_field_and_rejects_tamper() {
    let validator = OptimizationValidatorIdentity::from_canonical_bytes(b"validator-a");
    let base = OptimizationDecisionRecord::new(
        OptimizationUnitIdentity::from_canonical_bytes(b"input-a"),
        OptimizationCandidateIdentity::from_canonical_bytes(b"candidate-a"),
        rule(b"rule-a"),
        OptimizationCandidateVerdict::Applied,
        AnalysisSet::new([AnalysisKind::ScalarConstants]),
        vec![fact(b"fact-a")],
        Some(validator),
    )
    .unwrap();
    let variants = [
        OptimizationDecisionRecord::new(
            OptimizationUnitIdentity::from_canonical_bytes(b"input-b"),
            base.candidate(),
            base.rule(),
            base.verdict(),
            base.consumed_analyses(),
            base.consumed_facts().to_vec(),
            base.validator(),
        )
        .unwrap(),
        OptimizationDecisionRecord::new(
            base.input(),
            OptimizationCandidateIdentity::from_canonical_bytes(b"candidate-b"),
            base.rule(),
            base.verdict(),
            base.consumed_analyses(),
            base.consumed_facts().to_vec(),
            base.validator(),
        )
        .unwrap(),
        OptimizationDecisionRecord::new(
            base.input(),
            base.candidate(),
            rule(b"rule-b"),
            base.verdict(),
            base.consumed_analyses(),
            base.consumed_facts().to_vec(),
            base.validator(),
        )
        .unwrap(),
        OptimizationDecisionRecord::new(
            base.input(),
            base.candidate(),
            base.rule(),
            OptimizationCandidateVerdict::Skipped(OptimizationReasonCode::Superseded),
            base.consumed_analyses(),
            base.consumed_facts().to_vec(),
            base.validator(),
        )
        .unwrap(),
        OptimizationDecisionRecord::new(
            base.input(),
            base.candidate(),
            base.rule(),
            base.verdict(),
            AnalysisSet::new([AnalysisKind::ValueRanges]),
            base.consumed_facts().to_vec(),
            base.validator(),
        )
        .unwrap(),
        OptimizationDecisionRecord::new(
            base.input(),
            base.candidate(),
            base.rule(),
            base.verdict(),
            base.consumed_analyses(),
            vec![fact(b"fact-b")],
            base.validator(),
        )
        .unwrap(),
        OptimizationDecisionRecord::new(
            base.input(),
            base.candidate(),
            base.rule(),
            base.verdict(),
            base.consumed_analyses(),
            base.consumed_facts().to_vec(),
            Some(OptimizationValidatorIdentity::from_canonical_bytes(
                b"validator-b",
            )),
        )
        .unwrap(),
    ];
    assert!(variants
        .iter()
        .all(|variant| variant.identity() != base.identity()));

    let mut tampered = base.encode();
    tampered[12] ^= 1;
    assert_eq!(
        OptimizationDecisionRecord::decode(&tampered),
        Err(OptimizationManifestDecodeError::DecisionIdentityMismatch)
    );
}

#[test]
fn consumed_fact_references_must_be_canonical_and_known() {
    let first = fact(b"first");
    let second = fact(b"second");
    let mut canonical = vec![first, second];
    canonical.sort_unstable();
    let duplicate = vec![canonical[0], canonical[0]];
    assert_eq!(
        OptimizationDecisionRecord::new(
            OptimizationUnitIdentity::from_canonical_bytes(b"input"),
            OptimizationCandidateIdentity::from_canonical_bytes(b"candidate"),
            rule(b"rule"),
            OptimizationCandidateVerdict::Skipped(OptimizationReasonCode::Superseded),
            AnalysisSet::default(),
            duplicate,
            None,
        ),
        Err(InvalidOptimizationManifestRecord::NonCanonicalConsumedFacts)
    );
    canonical.reverse();
    assert_eq!(
        OptimizationDecisionRecord::new(
            OptimizationUnitIdentity::from_canonical_bytes(b"input"),
            OptimizationCandidateIdentity::from_canonical_bytes(b"candidate"),
            rule(b"rule"),
            OptimizationCandidateVerdict::Skipped(OptimizationReasonCode::Superseded),
            AnalysisSet::default(),
            canonical,
            None,
        ),
        Err(InvalidOptimizationManifestRecord::NonCanonicalConsumedFacts)
    );

    let record = decision(rule(b"rule"));
    let mut unknown = record.encode();
    unknown[154] = 99;
    assert_eq!(
        OptimizationDecisionRecord::decode(&unknown),
        Err(OptimizationManifestDecodeError::UnknownFactReference(99))
    );

    let mut mixed = vec![
        fact(b"operand"),
        obligation_fact(b"proof"),
        ownership_fact(b"ownership"),
    ];
    mixed.sort_unstable();
    let mixed_record = OptimizationDecisionRecord::new(
        OptimizationUnitIdentity::from_canonical_bytes(b"mixed-input"),
        OptimizationCandidateIdentity::from_canonical_bytes(b"mixed-candidate"),
        rule(b"mixed-rule"),
        OptimizationCandidateVerdict::Applied,
        AnalysisSet::new([AnalysisKind::ScalarConstants]),
        mixed,
        Some(OptimizationValidatorIdentity::from_canonical_bytes(
            b"mixed-validator",
        )),
    )
    .unwrap();
    assert_eq!(
        OptimizationDecisionRecord::decode(&mixed_record.encode()),
        Ok(mixed_record)
    );
}
