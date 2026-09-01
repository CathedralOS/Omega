use super::*;

fn rule(kind: IntegerRangePairComparisonKind) -> &'static dyn PsiOptimizationRule {
    match kind {
        IntegerRangePairComparisonKind::Equal => &IntegerEqualRangeRangeRule,
        IntegerRangePairComparisonKind::LessThan => &IntegerLessThanRangeRangeRule,
        IntegerRangePairComparisonKind::LessOrEqual => &IntegerLessOrEqualRangeRangeRule,
    }
}

fn propose_one(
    unit: &PsiOptimizationUnit,
    kind: IntegerRangePairComparisonKind,
) -> PsiRewriteCandidate {
    let ranges = compute_analysis(unit, AnalysisKind::ValueRanges).unwrap();
    let candidates = rule(kind)
        .propose(unit, RuleAnalysisView::new(&[ranges]))
        .unwrap();
    assert_eq!(candidates.len(), 1);
    candidates.into_iter().next().unwrap()
}

fn assert_fold(
    unit: PsiOptimizationUnit,
    kind: IntegerRangePairComparisonKind,
    expected: bool,
    expected_fact_count: usize,
) {
    let candidate = propose_one(&unit, kind);
    assert_eq!(
        candidate.safety_class(),
        OptimizationSafetyClass::ProofCertified
    );
    candidate
        .scalar_evaluation_witness()
        .and_then(IntegerEvaluationWitness::range_against_range)
        .expect("range-pair candidate retains both range facts");
    assert_eq!(candidate.consumed_facts().len(), expected_fact_count);
    let PsiRewritePatch::ReplaceBooleanOperationWithConstant(patch) = candidate.patch() else {
        unreachable!()
    };
    assert_eq!(patch.constant, expected);
    let accepted = validate_boolean_evaluation_candidate(&unit, &candidate).unwrap();
    let node = &accepted.unit().functions[0].blocks[0].nodes
        [usize::try_from(patch.location.node).unwrap()];
    assert!(matches!(
        node.operation,
        AbstractOperation::BooleanConstant { value, .. } if value == expected
    ));
}

#[test]
fn every_range_pair_rule_proposes_and_validates_true_and_false_results() {
    let unsigned8 = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let signed8 = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let cases = [
        (
            IntegerRangePairComparisonKind::Equal,
            unsigned8,
            ProofRangeKind::Zero,
            ProofRangeKind::Zero,
            true,
        ),
        (
            IntegerRangePairComparisonKind::Equal,
            unsigned8,
            ProofRangeKind::Zero,
            ProofRangeKind::Nonzero,
            false,
        ),
        (
            IntegerRangePairComparisonKind::LessThan,
            unsigned8,
            ProofRangeKind::Zero,
            ProofRangeKind::Nonzero,
            true,
        ),
        (
            IntegerRangePairComparisonKind::LessThan,
            signed8,
            ProofRangeKind::Zero,
            ProofRangeKind::Zero,
            false,
        ),
        (
            IntegerRangePairComparisonKind::LessOrEqual,
            signed8,
            ProofRangeKind::Zero,
            ProofRangeKind::Zero,
            true,
        ),
        (
            IntegerRangePairComparisonKind::LessOrEqual,
            unsigned8,
            ProofRangeKind::Nonzero,
            ProofRangeKind::Zero,
            false,
        ),
    ];
    for (kind, scalar_type, left, right, expected) in cases {
        assert_fold(
            range_pair_comparison_unit(kind, scalar_type, left, right, false),
            kind,
            expected,
            2,
        );
    }
}

#[test]
fn every_range_pair_rule_declines_an_indeterminate_overlap() {
    let scalar_types = [
        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        IntegerType::new(IntegerSign::Signed, 8).unwrap(),
    ];
    for scalar_type in scalar_types {
        for kind in [
            IntegerRangePairComparisonKind::Equal,
            IntegerRangePairComparisonKind::LessThan,
            IntegerRangePairComparisonKind::LessOrEqual,
        ] {
            let unit = range_pair_comparison_unit(
                kind,
                scalar_type,
                ProofRangeKind::ZeroToThree,
                ProofRangeKind::ZeroToThree,
                false,
            );
            let ranges = compute_analysis(&unit, AnalysisKind::ValueRanges).unwrap();
            assert!(
                rule(kind)
                    .propose(&unit, RuleAnalysisView::new(&[ranges]))
                    .unwrap()
                    .is_empty(),
                "{kind:?} must decline overlapping ranges",
            );
        }
    }
}

#[test]
fn same_value_range_pair_rules_cover_their_complete_truth_table() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    for (kind, expected) in [
        (IntegerRangePairComparisonKind::Equal, true),
        (IntegerRangePairComparisonKind::LessThan, false),
        (IntegerRangePairComparisonKind::LessOrEqual, true),
    ] {
        assert_fold(
            range_pair_comparison_unit(
                kind,
                scalar_type,
                ProofRangeKind::ZeroToThree,
                ProofRangeKind::ZeroToThree,
                true,
            ),
            kind,
            expected,
            1,
        );
    }
}

#[test]
fn every_range_pair_rule_declines_every_other_operation_shape() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let kinds = [
        IntegerRangePairComparisonKind::Equal,
        IntegerRangePairComparisonKind::LessThan,
        IntegerRangePairComparisonKind::LessOrEqual,
    ];
    for (operation_index, operation_kind) in kinds.into_iter().enumerate() {
        let unit = range_pair_comparison_unit(
            operation_kind,
            scalar_type,
            ProofRangeKind::ZeroToThree,
            ProofRangeKind::ZeroToThree,
            true,
        );
        let ranges = compute_analysis(&unit, AnalysisKind::ValueRanges).unwrap();
        for (rule_index, candidate_kind) in kinds.into_iter().enumerate() {
            let candidates = rule(candidate_kind)
                .propose(&unit, RuleAnalysisView::new(&[ranges.clone()]))
                .unwrap();
            assert_eq!(
                candidates.len(),
                usize::from(operation_index == rule_index),
                "rule {candidate_kind:?} against operation {operation_kind:?}",
            );
        }
    }
}

#[test]
fn range_pair_validation_rejects_cross_rule_labels_and_wrong_results() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let kinds = [
        IntegerRangePairComparisonKind::Equal,
        IntegerRangePairComparisonKind::LessThan,
        IntegerRangePairComparisonKind::LessOrEqual,
    ];
    let contracts = kinds.map(|kind| rule(kind).contract());
    for (source_index, kind) in kinds.into_iter().enumerate() {
        let unit = range_pair_comparison_unit(
            kind,
            scalar_type,
            ProofRangeKind::ZeroToThree,
            ProofRangeKind::ZeroToThree,
            true,
        );
        let candidate = propose_one(&unit, kind);
        let witness = candidate.scalar_evaluation_witness().unwrap();
        let PsiRewritePatch::ReplaceBooleanOperationWithConstant(patch) = candidate.patch() else {
            unreachable!()
        };
        for (contract_index, contract) in contracts.into_iter().enumerate() {
            if contract_index == source_index {
                continue;
            }
            let relabelled = PsiRewriteCandidate::new_boolean_evaluation(
                unit.identity,
                contract,
                candidate.affected_blocks().to_vec(),
                Vec::new(),
                candidate.provenance().to_vec(),
                witness,
                candidate.predicted_cost_delta(),
                patch,
            )
            .unwrap();
            assert!(validate_boolean_evaluation_candidate(&unit, &relabelled).is_err());
        }
        let mut wrong_patch = patch;
        wrong_patch.constant = !wrong_patch.constant;
        let wrong_result = PsiRewriteCandidate::new_boolean_evaluation(
            unit.identity,
            contracts[source_index],
            candidate.affected_blocks().to_vec(),
            Vec::new(),
            candidate.provenance().to_vec(),
            witness,
            candidate.predicted_cost_delta(),
            wrong_patch,
        )
        .unwrap();
        assert!(matches!(
            validate_boolean_evaluation_candidate(&unit, &wrong_result),
            Err(OptimizationUnitValidationError::CandidateEvaluationMismatch)
        ));
    }
}

#[test]
fn legacy_proof_range_pair_fixture_remains_a_valid_two_fact_fold() {
    let unit = proof_range_pair_comparison_unit();
    let candidate = propose_one(&unit, IntegerRangePairComparisonKind::LessOrEqual);
    assert_eq!(candidate.consumed_facts().len(), 2);
    validate_boolean_evaluation_candidate(&unit, &candidate).unwrap();
}

#[test]
fn range_pair_validation_rejects_unknown_identity_and_contract_supersets() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let kind = IntegerRangePairComparisonKind::Equal;
    let unit = range_pair_comparison_unit(
        kind,
        scalar_type,
        ProofRangeKind::Zero,
        ProofRangeKind::Zero,
        false,
    );
    let candidate = propose_one(&unit, kind);
    let base = rule(kind).contract();
    let witness = candidate.scalar_evaluation_witness().unwrap();
    let PsiRewritePatch::ReplaceBooleanOperationWithConstant(patch) = candidate.patch() else {
        unreachable!()
    };
    let contracts = [
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(b"omega.psi-rule.unknown-range-pair.v1"),
            base.pass(),
            base.version(),
            base.required_analyses(),
            base.invalidated_analyses(),
            base.safety_class(),
        )
        .unwrap(),
        OptimizationRuleContract::new(
            base.identity(),
            base.pass(),
            base.version(),
            omega_optimization_core::AnalysisSet::new([
                AnalysisKind::ScalarConstants,
                AnalysisKind::ValueRanges,
            ]),
            base.invalidated_analyses(),
            base.safety_class(),
        )
        .unwrap(),
        OptimizationRuleContract::new(
            base.identity(),
            base.pass(),
            base.version(),
            base.required_analyses(),
            omega_optimization_core::AnalysisInvalidationSet::new([
                AnalysisKind::Dominators,
                AnalysisKind::UseDefinition,
            ]),
            base.safety_class(),
        )
        .unwrap(),
    ];
    for contract in contracts {
        let forged = PsiRewriteCandidate::new_boolean_evaluation(
            unit.identity,
            contract,
            candidate.affected_blocks().to_vec(),
            Vec::new(),
            candidate.provenance().to_vec(),
            witness,
            candidate.predicted_cost_delta(),
            patch,
        )
        .unwrap();
        assert!(validate_boolean_evaluation_candidate(&unit, &forged).is_err());
    }

    let (left_range_fact, _) = witness.range_against_range().unwrap();
    let foreign_range = PsiRewriteCandidate::new_boolean_evaluation(
        unit.identity,
        base,
        candidate.affected_blocks().to_vec(),
        Vec::new(),
        candidate.provenance().to_vec(),
        IntegerEvaluationWitness::RangeAgainstRange {
            left_range_fact,
            right_range_fact: omega_optimization_core::ValueRangeFactIdentity::from_canonical_bytes(
                b"foreign-range-pair-fact",
            ),
        },
        candidate.predicted_cost_delta(),
        patch,
    )
    .unwrap();
    assert!(matches!(
        validate_boolean_evaluation_candidate(&unit, &foreign_range),
        Err(OptimizationUnitValidationError::CurrentValueRangeFactMismatch)
    ));
}
