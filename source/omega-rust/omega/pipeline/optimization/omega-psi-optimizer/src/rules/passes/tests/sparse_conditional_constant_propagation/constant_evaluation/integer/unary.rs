//! Exact cast, widen, and bitwise-not constant evaluation.

use super::*;

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
