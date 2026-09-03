//! Boolean-result constant evaluation by exact rule identity.

use super::*;

fn propose_one(unit: &PsiOptimizationUnit, rule: &dyn PsiOptimizationRule) -> PsiRewriteCandidate {
    let constants = compute_analysis(unit, AnalysisKind::ScalarConstants).unwrap();
    let candidates = rule
        .propose(unit, RuleAnalysisView::new(&[constants]))
        .unwrap();
    assert_eq!(candidates.len(), 1);
    candidates.into_iter().next().unwrap()
}

fn assert_exact_boolean_fold(
    unit: PsiOptimizationUnit,
    rule: &dyn PsiOptimizationRule,
    expected: bool,
    unary: bool,
) {
    let candidate = propose_one(&unit, rule);
    assert_eq!(
        candidate.safety_class(),
        OptimizationSafetyClass::ExactOperationSemantics
    );
    assert_eq!(
        matches!(
            candidate.scalar_evaluation_witness().unwrap(),
            IntegerEvaluationWitness::Unary { .. }
        ),
        unary,
    );
    let accepted = validate_boolean_evaluation_candidate(&unit, &candidate).unwrap();
    assert!(matches!(
        accepted.unit().functions[0].blocks[0].nodes[2].operation,
        AbstractOperation::BooleanConstant { value, .. } if value == expected
    ));
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

#[test]
fn boolean_literal_rules_cover_both_truth_boundaries() {
    let cases: [(
        BooleanFixtureKind,
        &dyn PsiOptimizationRule,
        bool,
        bool,
        bool,
    ); 4] = [
        (
            BooleanFixtureKind::Not,
            &BooleanNotConstantsRule,
            false,
            false,
            true,
        ),
        (
            BooleanFixtureKind::Not,
            &BooleanNotConstantsRule,
            true,
            false,
            false,
        ),
        (
            BooleanFixtureKind::Equal,
            &BooleanEqualConstantsRule,
            false,
            false,
            true,
        ),
        (
            BooleanFixtureKind::Equal,
            &BooleanEqualConstantsRule,
            true,
            false,
            false,
        ),
    ];
    for (kind, rule, left, right, expected) in cases {
        assert_exact_boolean_fold(
            boolean_constant_unit(kind, left, right),
            rule,
            expected,
            kind == BooleanFixtureKind::Not,
        );
    }
}

#[test]
fn integer_comparison_rules_cover_signed_and_unsigned_boundaries() {
    let unsigned8 = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let signed8 = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let u = IntegerValue::Unsigned;
    let s = IntegerValue::Signed;
    let cases: [(
        ComparisonFixtureKind,
        &dyn PsiOptimizationRule,
        IntegerType,
        IntegerValue,
        IntegerValue,
        bool,
    ); 6] = [
        (
            ComparisonFixtureKind::Equal,
            &IntegerEqualConstantsRule,
            unsigned8,
            u(0),
            u(255),
            false,
        ),
        (
            ComparisonFixtureKind::Equal,
            &IntegerEqualConstantsRule,
            signed8,
            s(-128),
            s(-128),
            true,
        ),
        (
            ComparisonFixtureKind::LessThan,
            &IntegerLessThanConstantsRule,
            unsigned8,
            u(255),
            u(0),
            false,
        ),
        (
            ComparisonFixtureKind::LessThan,
            &IntegerLessThanConstantsRule,
            signed8,
            s(-128),
            s(127),
            true,
        ),
        (
            ComparisonFixtureKind::LessOrEqual,
            &IntegerLessOrEqualConstantsRule,
            unsigned8,
            u(255),
            u(255),
            true,
        ),
        (
            ComparisonFixtureKind::LessOrEqual,
            &IntegerLessOrEqualConstantsRule,
            signed8,
            s(127),
            s(-128),
            false,
        ),
    ];
    for (kind, rule, scalar_type, left, right, expected) in cases {
        assert_exact_boolean_fold(
            integer_comparison_constant_unit(kind, scalar_type, left, right),
            rule,
            expected,
            false,
        );
    }
}

#[test]
fn every_boolean_constant_rule_declines_every_other_operation_kind() {
    let unsigned8 = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let units = [
        boolean_constant_unit(BooleanFixtureKind::Not, true, false),
        boolean_constant_unit(BooleanFixtureKind::Equal, true, false),
        integer_comparison_constant_unit(
            ComparisonFixtureKind::Equal,
            unsigned8,
            IntegerValue::Unsigned(7),
            IntegerValue::Unsigned(8),
        ),
        integer_comparison_constant_unit(
            ComparisonFixtureKind::LessThan,
            unsigned8,
            IntegerValue::Unsigned(7),
            IntegerValue::Unsigned(8),
        ),
        integer_comparison_constant_unit(
            ComparisonFixtureKind::LessOrEqual,
            unsigned8,
            IntegerValue::Unsigned(7),
            IntegerValue::Unsigned(8),
        ),
    ];
    let rules: [&dyn PsiOptimizationRule; 5] = [
        &BooleanNotConstantsRule,
        &BooleanEqualConstantsRule,
        &IntegerEqualConstantsRule,
        &IntegerLessThanConstantsRule,
        &IntegerLessOrEqualConstantsRule,
    ];
    for (operation_index, unit) in units.iter().enumerate() {
        let constants = compute_analysis(unit, AnalysisKind::ScalarConstants).unwrap();
        for (rule_index, rule) in rules.iter().enumerate() {
            let candidates = rule
                .propose(
                    unit,
                    RuleAnalysisView::new(std::slice::from_ref(&constants)),
                )
                .unwrap();
            assert_eq!(
                candidates.len(),
                usize::from(operation_index == rule_index),
                "rule {rule_index} against operation {operation_index}",
            );
        }
    }
}

#[test]
fn boolean_validation_rejects_every_cross_rule_contract_and_unknown_identity() {
    let unsigned8 = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let units = [
        boolean_constant_unit(BooleanFixtureKind::Not, true, false),
        boolean_constant_unit(BooleanFixtureKind::Equal, true, false),
        integer_comparison_constant_unit(
            ComparisonFixtureKind::Equal,
            unsigned8,
            IntegerValue::Unsigned(7),
            IntegerValue::Unsigned(8),
        ),
        integer_comparison_constant_unit(
            ComparisonFixtureKind::LessThan,
            unsigned8,
            IntegerValue::Unsigned(7),
            IntegerValue::Unsigned(8),
        ),
        integer_comparison_constant_unit(
            ComparisonFixtureKind::LessOrEqual,
            unsigned8,
            IntegerValue::Unsigned(7),
            IntegerValue::Unsigned(8),
        ),
    ];
    let rules: [&dyn PsiOptimizationRule; 5] = [
        &BooleanNotConstantsRule,
        &BooleanEqualConstantsRule,
        &IntegerEqualConstantsRule,
        &IntegerLessThanConstantsRule,
        &IntegerLessOrEqualConstantsRule,
    ];
    let contracts = [
        BooleanNotConstantsRule::contract(),
        BooleanEqualConstantsRule::contract(),
        IntegerEqualConstantsRule::contract(),
        IntegerLessThanConstantsRule::contract(),
        IntegerLessOrEqualConstantsRule::contract(),
    ];
    for (rule_index, (unit, rule)) in units.into_iter().zip(rules).enumerate() {
        let candidate = propose_one(&unit, rule);
        let PsiRewritePatch::ReplaceBooleanOperationWithConstant(patch) = candidate.patch() else {
            unreachable!()
        };
        for (contract_index, wrong_contract) in contracts.iter().copied().enumerate() {
            if contract_index == rule_index {
                continue;
            }
            let relabelled = forge_candidate(
                &unit,
                &candidate,
                wrong_contract,
                candidate.scalar_evaluation_witness().unwrap(),
                patch,
            );
            assert!(matches!(
                validate_boolean_evaluation_candidate(&unit, &relabelled),
                Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch)
            ));
        }
        let mut wrong_result_patch = patch;
        wrong_result_patch.constant = !wrong_result_patch.constant;
        let wrong_result = forge_candidate(
            &unit,
            &candidate,
            contracts[rule_index],
            candidate.scalar_evaluation_witness().unwrap(),
            wrong_result_patch,
        );
        assert!(matches!(
            validate_boolean_evaluation_candidate(&unit, &wrong_result),
            Err(OptimizationUnitValidationError::CandidateEvaluationMismatch)
        ));
    }

    let unit = boolean_constant_unit(BooleanFixtureKind::Not, true, false);
    let candidate = propose_one(&unit, &BooleanNotConstantsRule);
    let base = BooleanNotConstantsRule::contract();
    let unknown = OptimizationRuleContract::new(
        OptimizationRuleIdentity::from_canonical_bytes(b"omega.psi-rule.unknown-boolean-fold.v1"),
        base.pass(),
        base.version(),
        base.required_analyses(),
        base.invalidated_analyses(),
        base.safety_class(),
    )
    .unwrap();
    let PsiRewritePatch::ReplaceBooleanOperationWithConstant(patch) = candidate.patch() else {
        unreachable!()
    };
    let relabelled = forge_candidate(
        &unit,
        &candidate,
        unknown,
        candidate.scalar_evaluation_witness().unwrap(),
        patch,
    );
    assert!(matches!(
        validate_boolean_evaluation_candidate(&unit, &relabelled),
        Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch)
    ));
}

#[test]
fn boolean_validation_rejects_unary_and_binary_witness_fact_corruption() {
    let unary_unit = boolean_constant_unit(BooleanFixtureKind::Not, true, false);
    let unary = propose_one(&unary_unit, &BooleanNotConstantsRule);
    let IntegerEvaluationWitness::Unary { operand_fact } =
        unary.scalar_evaluation_witness().unwrap()
    else {
        unreachable!()
    };
    let PsiRewritePatch::ReplaceBooleanOperationWithConstant(unary_patch) = unary.patch() else {
        unreachable!()
    };
    let wrong_shape = forge_candidate(
        &unary_unit,
        &unary,
        BooleanNotConstantsRule::contract(),
        IntegerEvaluationWitness::Binary {
            left_fact: operand_fact,
            right_fact: operand_fact,
        },
        unary_patch,
    );
    assert!(matches!(
        validate_boolean_evaluation_candidate(&unary_unit, &wrong_shape),
        Err(OptimizationUnitValidationError::CandidateOperandFactMismatch)
    ));
    let wrong_fact = forge_candidate(
        &unary_unit,
        &unary,
        BooleanNotConstantsRule::contract(),
        IntegerEvaluationWitness::Unary {
            operand_fact: ScalarConstantFactIdentity::from_canonical_bytes(
                b"foreign boolean literal fact",
            ),
        },
        unary_patch,
    );
    assert!(matches!(
        validate_boolean_evaluation_candidate(&unary_unit, &wrong_fact),
        Err(OptimizationUnitValidationError::CandidateOperandFactMismatch)
    ));

    let binary_unit = boolean_constant_unit(BooleanFixtureKind::Equal, true, false);
    let binary = propose_one(&binary_unit, &BooleanEqualConstantsRule);
    let IntegerEvaluationWitness::Binary { left_fact, .. } =
        binary.scalar_evaluation_witness().unwrap()
    else {
        unreachable!()
    };
    let PsiRewritePatch::ReplaceBooleanOperationWithConstant(binary_patch) = binary.patch() else {
        unreachable!()
    };
    let wrong_shape = forge_candidate(
        &binary_unit,
        &binary,
        BooleanEqualConstantsRule::contract(),
        IntegerEvaluationWitness::Unary {
            operand_fact: left_fact,
        },
        binary_patch,
    );
    assert!(matches!(
        validate_boolean_evaluation_candidate(&binary_unit, &wrong_shape),
        Err(OptimizationUnitValidationError::CandidateOperandFactMismatch)
    ));
    let wrong_fact = forge_candidate(
        &binary_unit,
        &binary,
        BooleanEqualConstantsRule::contract(),
        IntegerEvaluationWitness::Binary {
            left_fact,
            right_fact: ScalarConstantFactIdentity::from_canonical_bytes(
                b"foreign equality literal fact",
            ),
        },
        binary_patch,
    );
    assert!(matches!(
        validate_boolean_evaluation_candidate(&binary_unit, &wrong_fact),
        Err(OptimizationUnitValidationError::CandidateOperandFactMismatch)
    ));
}
