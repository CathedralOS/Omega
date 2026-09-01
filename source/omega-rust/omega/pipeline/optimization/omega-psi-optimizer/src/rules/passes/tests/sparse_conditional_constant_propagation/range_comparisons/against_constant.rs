use super::*;

fn rule(kind: IntegerRangeComparisonKind) -> &'static dyn PsiOptimizationRule {
    match kind {
        IntegerRangeComparisonKind::RangeEqualConstant => &IntegerEqualRangeConstantRule,
        IntegerRangeComparisonKind::ConstantEqualRange => &IntegerEqualConstantRangeRule,
        IntegerRangeComparisonKind::RangeLessThanConstant => &IntegerLessThanRangeConstantRule,
        IntegerRangeComparisonKind::ConstantLessThanRange => &IntegerLessThanConstantRangeRule,
        IntegerRangeComparisonKind::RangeLessOrEqualConstant => {
            &IntegerLessOrEqualRangeConstantRule
        }
        IntegerRangeComparisonKind::ConstantLessOrEqualRange => {
            &IntegerLessOrEqualConstantRangeRule
        }
    }
}

fn propose_one(
    unit: &PsiOptimizationUnit,
    kind: IntegerRangeComparisonKind,
) -> PsiRewriteCandidate {
    let constants = compute_analysis(unit, AnalysisKind::ScalarConstants).unwrap();
    let ranges = compute_analysis(unit, AnalysisKind::ValueRanges).unwrap();
    let candidates = rule(kind)
        .propose(unit, RuleAnalysisView::new(&[constants, ranges]))
        .unwrap();
    assert_eq!(candidates.len(), 1);
    candidates.into_iter().next().unwrap()
}

fn forge_candidate(
    unit: &PsiOptimizationUnit,
    source: &PsiRewriteCandidate,
    contract: OptimizationRuleContract,
    witness: IntegerEvaluationWitness,
    patch: BooleanConstantRewrite,
) -> PsiRewriteCandidate {
    PsiRewriteCandidate::new_boolean_evaluation(
        unit.identity,
        contract,
        source.affected_blocks().to_vec(),
        Vec::new(),
        source.provenance().to_vec(),
        witness,
        source.predicted_cost_delta(),
        patch,
    )
    .unwrap()
}

fn assert_fold(unit: PsiOptimizationUnit, kind: IntegerRangeComparisonKind, expected: bool) {
    let candidate = propose_one(&unit, kind);
    assert_eq!(
        candidate.safety_class(),
        OptimizationSafetyClass::ProofCertified
    );
    candidate
        .scalar_evaluation_witness()
        .and_then(IntegerEvaluationWitness::range_against_constant)
        .expect("range/literal candidate retains both exact facts");
    assert_eq!(candidate.consumed_facts().len(), 2);
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
fn every_range_literal_rule_proposes_and_validates_true_and_false_results() {
    let unsigned8 = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let signed8 = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let u = IntegerValue::Unsigned;
    let s = IntegerValue::Signed;
    let cases = [
        (
            IntegerRangeComparisonKind::RangeLessThanConstant,
            unsigned8,
            ProofRangeKind::ZeroToThree,
            u(4),
            true,
        ),
        (
            IntegerRangeComparisonKind::RangeLessThanConstant,
            unsigned8,
            ProofRangeKind::ZeroToThree,
            u(0),
            false,
        ),
        (
            IntegerRangeComparisonKind::ConstantLessThanRange,
            unsigned8,
            ProofRangeKind::Nonzero,
            u(0),
            true,
        ),
        (
            IntegerRangeComparisonKind::ConstantLessThanRange,
            unsigned8,
            ProofRangeKind::ZeroToThree,
            u(3),
            false,
        ),
        (
            IntegerRangeComparisonKind::RangeLessOrEqualConstant,
            unsigned8,
            ProofRangeKind::ZeroToThree,
            u(3),
            true,
        ),
        (
            IntegerRangeComparisonKind::RangeLessOrEqualConstant,
            signed8,
            ProofRangeKind::ZeroToThree,
            s(-1),
            false,
        ),
        (
            IntegerRangeComparisonKind::ConstantLessOrEqualRange,
            unsigned8,
            ProofRangeKind::ZeroToThree,
            u(0),
            true,
        ),
        (
            IntegerRangeComparisonKind::ConstantLessOrEqualRange,
            unsigned8,
            ProofRangeKind::ZeroToThree,
            u(4),
            false,
        ),
        (
            IntegerRangeComparisonKind::RangeEqualConstant,
            unsigned8,
            ProofRangeKind::Zero,
            u(0),
            true,
        ),
        (
            IntegerRangeComparisonKind::RangeEqualConstant,
            unsigned8,
            ProofRangeKind::ZeroToThree,
            u(4),
            false,
        ),
        (
            IntegerRangeComparisonKind::ConstantEqualRange,
            signed8,
            ProofRangeKind::Zero,
            s(0),
            true,
        ),
        (
            IntegerRangeComparisonKind::ConstantEqualRange,
            signed8,
            ProofRangeKind::ZeroToThree,
            s(4),
            false,
        ),
    ];
    for (kind, scalar_type, range_kind, constant, expected) in cases {
        assert_fold(
            range_constant_comparison_unit(kind, scalar_type, range_kind, constant),
            kind,
            expected,
        );
    }
}

#[test]
fn every_range_literal_rule_declines_an_indeterminate_overlap() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let u = IntegerValue::Unsigned;
    let cases = [
        (IntegerRangeComparisonKind::RangeLessThanConstant, u(2)),
        (IntegerRangeComparisonKind::ConstantLessThanRange, u(0)),
        (IntegerRangeComparisonKind::RangeLessOrEqualConstant, u(0)),
        (IntegerRangeComparisonKind::ConstantLessOrEqualRange, u(3)),
        (IntegerRangeComparisonKind::RangeEqualConstant, u(2)),
        (IntegerRangeComparisonKind::ConstantEqualRange, u(2)),
    ];
    for (kind, constant) in cases {
        let unit = range_constant_comparison_unit(
            kind,
            scalar_type,
            ProofRangeKind::ZeroToThree,
            constant,
        );
        let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        let ranges = compute_analysis(&unit, AnalysisKind::ValueRanges).unwrap();
        assert!(
            rule(kind)
                .propose(&unit, RuleAnalysisView::new(&[constants, ranges]))
                .unwrap()
                .is_empty(),
            "{kind:?} must decline a non-universal result",
        );
    }
}

#[test]
fn every_range_literal_rule_declines_every_other_operation_shape() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let kinds = [
        IntegerRangeComparisonKind::RangeLessThanConstant,
        IntegerRangeComparisonKind::ConstantLessThanRange,
        IntegerRangeComparisonKind::RangeLessOrEqualConstant,
        IntegerRangeComparisonKind::ConstantLessOrEqualRange,
        IntegerRangeComparisonKind::RangeEqualConstant,
        IntegerRangeComparisonKind::ConstantEqualRange,
    ];
    let constants = [
        IntegerValue::Unsigned(4),
        IntegerValue::Unsigned(0),
        IntegerValue::Unsigned(3),
        IntegerValue::Unsigned(0),
        IntegerValue::Unsigned(0),
        IntegerValue::Unsigned(0),
    ];
    let ranges = [
        ProofRangeKind::ZeroToThree,
        ProofRangeKind::Nonzero,
        ProofRangeKind::ZeroToThree,
        ProofRangeKind::ZeroToThree,
        ProofRangeKind::Zero,
        ProofRangeKind::Zero,
    ];
    for (operation_index, ((operation_kind, constant), range_kind)) in
        kinds.into_iter().zip(constants).zip(ranges).enumerate()
    {
        let unit =
            range_constant_comparison_unit(operation_kind, scalar_type, range_kind, constant);
        let scalar_constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        let value_ranges = compute_analysis(&unit, AnalysisKind::ValueRanges).unwrap();
        for (rule_index, candidate_kind) in kinds.into_iter().enumerate() {
            let candidates = rule(candidate_kind)
                .propose(
                    &unit,
                    RuleAnalysisView::new(&[scalar_constants.clone(), value_ranges.clone()]),
                )
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
fn range_literal_validation_rejects_cross_rule_labels_and_wrong_results() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let kinds = [
        IntegerRangeComparisonKind::RangeLessThanConstant,
        IntegerRangeComparisonKind::ConstantLessThanRange,
        IntegerRangeComparisonKind::RangeLessOrEqualConstant,
        IntegerRangeComparisonKind::ConstantLessOrEqualRange,
        IntegerRangeComparisonKind::RangeEqualConstant,
        IntegerRangeComparisonKind::ConstantEqualRange,
    ];
    let contracts = kinds.map(|kind| rule(kind).contract());
    for (source_index, kind) in kinds.into_iter().enumerate() {
        let (range_kind, constant) = match kind {
            IntegerRangeComparisonKind::RangeLessThanConstant => {
                (ProofRangeKind::ZeroToThree, IntegerValue::Unsigned(4))
            }
            IntegerRangeComparisonKind::ConstantLessThanRange => {
                (ProofRangeKind::Nonzero, IntegerValue::Unsigned(0))
            }
            IntegerRangeComparisonKind::RangeLessOrEqualConstant => {
                (ProofRangeKind::ZeroToThree, IntegerValue::Unsigned(3))
            }
            IntegerRangeComparisonKind::ConstantLessOrEqualRange => {
                (ProofRangeKind::ZeroToThree, IntegerValue::Unsigned(0))
            }
            IntegerRangeComparisonKind::RangeEqualConstant
            | IntegerRangeComparisonKind::ConstantEqualRange => {
                (ProofRangeKind::Zero, IntegerValue::Unsigned(0))
            }
        };
        let unit = range_constant_comparison_unit(kind, scalar_type, range_kind, constant);
        let candidate = propose_one(&unit, kind);
        let witness = candidate.scalar_evaluation_witness().unwrap();
        let PsiRewritePatch::ReplaceBooleanOperationWithConstant(patch) = candidate.patch() else {
            unreachable!()
        };
        for (contract_index, contract) in contracts.into_iter().enumerate() {
            if contract_index == source_index {
                continue;
            }
            let relabelled = forge_candidate(&unit, &candidate, contract, witness, patch);
            assert!(validate_boolean_evaluation_candidate(&unit, &relabelled).is_err());
        }
        let mut wrong_patch = patch;
        wrong_patch.constant = !wrong_patch.constant;
        let wrong_result = forge_candidate(
            &unit,
            &candidate,
            contracts[source_index],
            witness,
            wrong_patch,
        );
        assert!(matches!(
            validate_boolean_evaluation_candidate(&unit, &wrong_result),
            Err(OptimizationUnitValidationError::CandidateEvaluationMismatch)
        ));
    }
}

#[test]
fn range_literal_validation_rejects_unknown_identity_and_contract_supersets() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let kind = IntegerRangeComparisonKind::RangeLessThanConstant;
    let unit = range_constant_comparison_unit(
        kind,
        scalar_type,
        ProofRangeKind::ZeroToThree,
        IntegerValue::Unsigned(4),
    );
    let candidate = propose_one(&unit, kind);
    let base = rule(kind).contract();
    let witness = candidate.scalar_evaluation_witness().unwrap();
    let PsiRewritePatch::ReplaceBooleanOperationWithConstant(patch) = candidate.patch() else {
        unreachable!()
    };
    let contracts = [
        OptimizationRuleContract::new(
            OptimizationRuleIdentity::from_canonical_bytes(
                b"omega.psi-rule.unknown-range-literal.v1",
            ),
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
                AnalysisKind::Dominators,
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
        let forged = forge_candidate(&unit, &candidate, contract, witness, patch);
        assert!(validate_boolean_evaluation_candidate(&unit, &forged).is_err());
    }

    let (_, constant_fact) = witness.range_against_constant().unwrap();
    let foreign_range = forge_candidate(
        &unit,
        &candidate,
        base,
        IntegerEvaluationWitness::RangeAgainstConstant {
            range_fact: omega_optimization_core::ValueRangeFactIdentity::from_canonical_bytes(
                b"foreign-range-fact",
            ),
            constant_fact,
        },
        patch,
    );
    assert!(matches!(
        validate_boolean_evaluation_candidate(&unit, &foreign_range),
        Err(OptimizationUnitValidationError::CurrentValueRangeFactMismatch)
    ));
}
