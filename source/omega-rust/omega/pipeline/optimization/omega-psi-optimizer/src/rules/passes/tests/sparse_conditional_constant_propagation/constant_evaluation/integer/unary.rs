//! Exact cast, widen, and bitwise-not constant evaluation.

use super::*;

#[test]
fn unary_integer_rules_preserve_signed_and_unsigned_endpoint_semantics() {
    let unsigned8 = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let unsigned16 = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
    let signed8 = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let signed16 = IntegerType::new(IntegerSign::Signed, 16).unwrap();
    let u = IntegerValue::Unsigned;
    let s = IntegerValue::Signed;
    let cases: [(
        UnaryConstantFixtureKind,
        &'static dyn PsiOptimizationRule,
        IntegerType,
        IntegerType,
        IntegerValue,
        IntegerValue,
    ); 8] = [
        (
            UnaryConstantFixtureKind::ExactCast,
            &ExactIntegerCastConstantsRule,
            unsigned16,
            unsigned8,
            u(255),
            u(255),
        ),
        (
            UnaryConstantFixtureKind::ExactCast,
            &ExactIntegerCastConstantsRule,
            signed16,
            signed8,
            s(-128),
            s(-128),
        ),
        (
            UnaryConstantFixtureKind::Widen,
            &IntegerWidenConstantsRule,
            unsigned8,
            unsigned16,
            u(255),
            u(255),
        ),
        (
            UnaryConstantFixtureKind::Widen,
            &IntegerWidenConstantsRule,
            signed8,
            signed16,
            s(-128),
            s(-128),
        ),
        (
            UnaryConstantFixtureKind::BitwiseNot,
            &IntegerBitwiseNotConstantsRule,
            unsigned8,
            unsigned8,
            u(0),
            u(255),
        ),
        (
            UnaryConstantFixtureKind::BitwiseNot,
            &IntegerBitwiseNotConstantsRule,
            unsigned8,
            unsigned8,
            u(255),
            u(0),
        ),
        (
            UnaryConstantFixtureKind::BitwiseNot,
            &IntegerBitwiseNotConstantsRule,
            signed8,
            signed8,
            s(0),
            s(-1),
        ),
        (
            UnaryConstantFixtureKind::BitwiseNot,
            &IntegerBitwiseNotConstantsRule,
            signed8,
            signed8,
            s(-128),
            s(127),
        ),
    ];
    for (kind, rule, source_type, target_type, constant, expected) in cases {
        let unit = unary_constant_unit(kind, source_type, target_type, constant);
        let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        let candidates = rule
            .propose(&unit, RuleAnalysisView::new(&[constants]))
            .unwrap();
        assert_eq!(candidates.len(), 1, "exact unary entrance {kind:?}");
        let expected_safety = if kind.proof_certified() {
            OptimizationSafetyClass::ProofCertified
        } else {
            OptimizationSafetyClass::ExactOperationSemantics
        };
        assert_eq!(candidates[0].safety_class(), expected_safety, "{kind:?}");
        assert_eq!(
            matches!(
                candidates[0].scalar_evaluation_witness().unwrap(),
                IntegerEvaluationWitness::ProofCertifiedUnary { .. }
            ),
            kind.proof_certified(),
            "unary witness class for {kind:?}",
        );
        let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
        assert!(
            matches!(
                accepted.unit().functions[0].blocks[0].nodes[1].operation,
                AbstractOperation::IntegerConstant {
                    scalar_type: ScalarType::Integer(actual_type),
                    value,
                    ..
                } if actual_type == target_type && value == expected
            ),
            "validated unary result for {kind:?}"
        );
    }
}

#[test]
fn unary_integer_rules_decline_every_other_operation_kind() {
    let unsigned8 = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let unsigned16 = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
    let u = IntegerValue::Unsigned;
    let cases: [(PsiOptimizationUnit, [&'static dyn PsiOptimizationRule; 2]); 3] = [
        (
            unary_constant_unit(
                UnaryConstantFixtureKind::ExactCast,
                unsigned16,
                unsigned8,
                u(7),
            ),
            [&IntegerWidenConstantsRule, &IntegerBitwiseNotConstantsRule],
        ),
        (
            unary_constant_unit(UnaryConstantFixtureKind::Widen, unsigned8, unsigned16, u(7)),
            [
                &ExactIntegerCastConstantsRule,
                &IntegerBitwiseNotConstantsRule,
            ],
        ),
        (
            unary_constant_unit(
                UnaryConstantFixtureKind::BitwiseNot,
                unsigned8,
                unsigned8,
                u(7),
            ),
            [&ExactIntegerCastConstantsRule, &IntegerWidenConstantsRule],
        ),
    ];
    for (unit, wrong_rules) in cases {
        let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        for rule in wrong_rules {
            assert!(
                rule.propose(&unit, RuleAnalysisView::new(&[constants.clone()]))
                    .unwrap()
                    .is_empty(),
                "an exact unary entrance must not claim another operation kind",
            );
        }
    }
}

#[test]
fn unary_integer_validation_rejects_relabelled_rule_contracts() {
    let unsigned8 = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let unsigned16 = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
    let u = IntegerValue::Unsigned;
    let cases: [(
        PsiOptimizationUnit,
        &'static dyn PsiOptimizationRule,
        OptimizationRuleContract,
    ); 3] = [
        (
            unary_constant_unit(
                UnaryConstantFixtureKind::ExactCast,
                unsigned16,
                unsigned8,
                u(7),
            ),
            &ExactIntegerCastConstantsRule,
            ExactIntegerAddConstantsRule::contract(),
        ),
        (
            unary_constant_unit(UnaryConstantFixtureKind::Widen, unsigned8, unsigned16, u(7)),
            &IntegerWidenConstantsRule,
            IntegerBitwiseNotConstantsRule::contract(),
        ),
        (
            unary_constant_unit(
                UnaryConstantFixtureKind::BitwiseNot,
                unsigned8,
                unsigned8,
                u(7),
            ),
            &IntegerBitwiseNotConstantsRule,
            IntegerWidenConstantsRule::contract(),
        ),
    ];
    for (unit, rule, wrong_contract) in cases {
        let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        let candidates = rule
            .propose(&unit, RuleAnalysisView::new(&[constants]))
            .unwrap();
        assert_eq!(candidates.len(), 1);
        let PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) = candidates[0].patch()
        else {
            unreachable!()
        };
        let relabelled = PsiRewriteCandidate::new_integer_evaluation(
            unit.identity,
            wrong_contract,
            vec![unit.functions[0].blocks[0].id],
            Vec::new(),
            candidates[0].provenance().to_vec(),
            candidates[0].scalar_evaluation_witness().unwrap(),
            -1,
            patch,
        )
        .unwrap();
        assert!(matches!(
            validate_integer_evaluation_candidate(&unit, &relabelled),
            Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch)
        ));
    }
}

#[test]
fn exact_cast_rule_uses_unary_evidence_and_target_integer_semantics() {
    let unit = exact_cast_unit(250);
    let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
    let candidates = ExactIntegerCastConstantsRule
        .propose(&unit, RuleAnalysisView::new(&[constants]))
        .unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].safety_class(),
        OptimizationSafetyClass::ProofCertified
    );
    assert!(matches!(
        candidates[0].scalar_evaluation_witness().unwrap(),
        IntegerEvaluationWitness::ProofCertifiedUnary { .. }
    ));
    let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
    let target_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    assert!(matches!(
        accepted.unit().functions[0].blocks[0].nodes[1].operation,
        AbstractOperation::IntegerConstant {
            scalar_type: ScalarType::Integer(scalar_type),
            value: IntegerValue::Unsigned(250),
            ..
        } if scalar_type == target_type
    ));

    let IntegerEvaluationWitness::ProofCertifiedUnary {
        operand_fact,
        obligation_fact,
    } = candidates[0].scalar_evaluation_witness().unwrap()
    else {
        unreachable!()
    };
    let omega_optimization_unit::PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) =
        candidates[0].patch()
    else {
        unreachable!()
    };
    let binary_witness = PsiRewriteCandidate::new_integer_evaluation(
        unit.identity,
        ExactIntegerCastConstantsRule::contract(),
        vec![unit.functions[0].blocks[0].id],
        Vec::new(),
        candidates[0].provenance().to_vec(),
        IntegerEvaluationWitness::ProofCertifiedBinary {
            left_fact: operand_fact,
            right_fact: operand_fact,
            obligation_fact,
        },
        -1,
        patch,
    )
    .unwrap();
    assert_eq!(binary_witness.consumed_facts().len(), 2);
    assert_ne!(binary_witness.identity(), candidates[0].identity());
    assert!(matches!(
            validate_integer_evaluation_candidate(&unit, &binary_witness),
            Err(omega_optimization_validation::OptimizationUnitValidationError::CandidateOperandFactMismatch)
        ));
}

#[test]
fn exact_cast_rule_declines_a_constant_outside_the_target_domain() {
    let unit = exact_cast_unit(300);
    let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
    assert!(
        ExactIntegerCastConstantsRule
            .propose(&unit, RuleAnalysisView::new(&[constants]))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn widen_and_bitwise_not_rules_reuse_typed_unary_evidence() {
    let cases: [(bool, &dyn PsiOptimizationRule, u128, u16); 2] = [
        (true, &IntegerWidenConstantsRule, 15, 16),
        (false, &IntegerBitwiseNotConstantsRule, 240, 8),
    ];
    for (widen, rule, expected, expected_bits) in cases {
        let unit = goal_free_unary_unit(widen);
        let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        let candidates = rule
            .propose(&unit, RuleAnalysisView::new(&[constants]))
            .unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].safety_class(),
            OptimizationSafetyClass::ExactOperationSemantics
        );
        assert!(matches!(
            candidates[0].scalar_evaluation_witness().unwrap(),
            IntegerEvaluationWitness::Unary { .. }
        ));
        let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
        assert!(matches!(
            accepted.unit().functions[0].blocks[0].nodes[1].operation,
            AbstractOperation::IntegerConstant {
                scalar_type: ScalarType::Integer(scalar_type),
                value: IntegerValue::Unsigned(value),
                ..
            } if value == expected && scalar_type.bits() == expected_bits
        ));
    }
}
