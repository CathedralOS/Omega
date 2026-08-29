//! Sparse conditional constant propagation and range-proof tests.

use super::*;

#[test]
fn selected_builtin_proposes_one_independently_validated_exact_fold() {
    let unit = exact_add_unit();
    let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
    let ranges = compute_analysis(&unit, AnalysisKind::ValueRanges).unwrap();
    let products = vec![constants, ranges];
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    assert_eq!(registry.len(), 39);
    let mut dispatched = 0usize;
    let mut candidates = Vec::new();
    for rule in registry.iter() {
        dispatched += 1;
        candidates.extend(
            rule.propose(&unit, RuleAnalysisView::new(&products))
                .unwrap(),
        );
    }
    assert_eq!(dispatched, registry.len());
    assert_eq!(candidates.len(), 1);
    let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
    assert!(matches!(
        accepted.unit().functions[0].blocks[0].nodes[2].operation,
        AbstractOperation::IntegerConstant {
            value: IntegerValue::Unsigned(15),
            ..
        }
    ));
}

#[test]
fn wrapping_and_saturating_rules_use_their_exact_declared_policies() {
    for (unit, saturating) in [(wrapping_add_unit(), false), (policy_add_unit(true), true)] {
        let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        let products = vec![constants];
        let candidates = if saturating {
            SaturatingIntegerAddConstantsRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
        } else {
            WrappingIntegerAddConstantsRule
                .propose(&unit, RuleAnalysisView::new(&products))
                .unwrap()
        };
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].safety_class(),
            OptimizationSafetyClass::ExactOperationSemantics
        );
        let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
        let expected = if saturating { 255 } else { 4 };
        assert!(matches!(
            accepted.unit().functions[0].blocks[0].nodes[2].operation,
            AbstractOperation::IntegerConstant {
                value: IntegerValue::Unsigned(value),
                ..
            } if value == expected
        ));
    }
}

#[test]
fn binary_bitwise_rules_fold_with_typed_psi_semantics() {
    let cases: [(BitwiseFixtureKind, &dyn PsiOptimizationRule, u128); 3] = [
        (BitwiseFixtureKind::And, &IntegerBitwiseAndConstantsRule, 8),
        (BitwiseFixtureKind::Or, &IntegerBitwiseOrConstantsRule, 14),
        (BitwiseFixtureKind::Xor, &IntegerBitwiseXorConstantsRule, 6),
    ];
    for (kind, rule, expected) in cases {
        let unit = bitwise_unit(kind);
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
            IntegerEvaluationWitness::Binary { .. }
        ));
        let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
        assert!(matches!(
            accepted.unit().functions[0].blocks[0].nodes[2].operation,
            AbstractOperation::IntegerConstant {
                value: IntegerValue::Unsigned(value),
                ..
            } if value == expected
        ));
    }
}

#[test]
fn propagated_block_parameter_fact_is_independently_reconstructed() {
    let unit = propagated_block_parameter_unit(true);
    let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
    let candidates = IntegerBitwiseNotConstantsRule
        .propose(&unit, RuleAnalysisView::new(&[constants]))
        .unwrap();
    assert_eq!(candidates.len(), 1);
    let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
    assert!(matches!(
        accepted.unit().functions[0].blocks[3].nodes[0].operation,
        AbstractOperation::IntegerConstant {
            value: IntegerValue::Unsigned(248),
            ..
        }
    ));
}

#[test]
fn proof_bearing_division_folds_only_when_the_declared_operation_is_defined() {
    let unit = exact_divide_unit(false);
    let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
    let candidates = ExactIntegerDivideConstantsRule
        .propose(&unit, RuleAnalysisView::new(&[constants]))
        .unwrap();
    assert_eq!(candidates.len(), 1);
    let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
    assert!(matches!(
        accepted.unit().functions[0].blocks[0].nodes[2].operation,
        AbstractOperation::IntegerConstant {
            value: IntegerValue::Unsigned(0),
            ..
        }
    ));

    let zero = exact_divide_unit(true);
    let constants = compute_analysis(&zero, AnalysisKind::ScalarConstants).unwrap();
    assert!(
        ExactIntegerDivideConstantsRule
            .propose(&zero, RuleAnalysisView::new(&[constants]))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn exact_and_wrapping_shift_rules_use_psi_integer_semantics() {
    let cases: [(ShiftFixtureKind, &dyn PsiOptimizationRule, u128, u128, u128); 4] = [
        (
            ShiftFixtureKind::ExactLeft,
            &ExactIntegerShiftLeftConstantsRule,
            7,
            2,
            28,
        ),
        (
            ShiftFixtureKind::ExactRight,
            &ExactIntegerShiftRightConstantsRule,
            7,
            2,
            1,
        ),
        (
            ShiftFixtureKind::WrappingLeft,
            &WrappingIntegerShiftLeftConstantsRule,
            250,
            2,
            232,
        ),
        (
            ShiftFixtureKind::WrappingRight,
            &WrappingIntegerShiftRightConstantsRule,
            250,
            2,
            62,
        ),
    ];
    for (kind, rule, value, count, expected) in cases {
        let unit = shift_unit(kind, value, count);
        let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        let candidates = rule
            .propose(&unit, RuleAnalysisView::new(&[constants]))
            .unwrap();
        assert_eq!(candidates.len(), 1);
        let expected_safety = if matches!(
            kind,
            ShiftFixtureKind::ExactLeft | ShiftFixtureKind::ExactRight
        ) {
            OptimizationSafetyClass::ProofCertified
        } else {
            OptimizationSafetyClass::ExactOperationSemantics
        };
        assert_eq!(candidates[0].safety_class(), expected_safety);
        let accepted = validate_integer_evaluation_candidate(&unit, &candidates[0]).unwrap();
        assert!(matches!(
            accepted.unit().functions[0].blocks[0].nodes[2].operation,
            AbstractOperation::IntegerConstant {
                value: IntegerValue::Unsigned(value),
                ..
            } if value == expected
        ));
    }
}

#[test]
fn exact_shift_left_declines_an_overflowing_constant_evaluation() {
    let unit = shift_unit(ShiftFixtureKind::ExactLeft, 250, 2);
    let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
    assert!(
        ExactIntegerShiftLeftConstantsRule
            .propose(&unit, RuleAnalysisView::new(&[constants]))
            .unwrap()
            .is_empty()
    );
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

#[test]
fn boolean_not_and_equal_use_typed_boolean_patches() {
    let cases: [(bool, &dyn PsiOptimizationRule); 2] = [
        (false, &BooleanNotConstantsRule),
        (true, &BooleanEqualConstantsRule),
    ];
    for (equal, rule) in cases {
        let unit = boolean_unit(equal);
        let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        let candidates = rule
            .propose(&unit, RuleAnalysisView::new(&[constants]))
            .unwrap();
        assert_eq!(candidates.len(), 1);
        assert!(matches!(
            candidates[0].patch(),
            omega_optimization_unit::PsiRewritePatch::ReplaceBooleanOperationWithConstant(_)
        ));
        let accepted = validate_boolean_evaluation_candidate(&unit, &candidates[0]).unwrap();
        assert!(matches!(
            accepted.unit().functions[0].blocks[0].nodes[2].operation,
            AbstractOperation::BooleanConstant { value: false, .. }
        ));
    }
}

#[test]
fn integer_comparison_rules_reconstruct_operand_types_and_boolean_results() {
    let cases: [(ComparisonFixtureKind, &dyn PsiOptimizationRule, bool); 3] = [
        (
            ComparisonFixtureKind::Equal,
            &IntegerEqualConstantsRule,
            false,
        ),
        (
            ComparisonFixtureKind::LessThan,
            &IntegerLessThanConstantsRule,
            true,
        ),
        (
            ComparisonFixtureKind::LessOrEqual,
            &IntegerLessOrEqualConstantsRule,
            true,
        ),
    ];
    for (kind, rule, expected) in cases {
        let unit = integer_comparison_unit(kind);
        let constants = compute_analysis(&unit, AnalysisKind::ScalarConstants).unwrap();
        let candidates = rule
            .propose(&unit, RuleAnalysisView::new(&[constants]))
            .unwrap();
        assert_eq!(candidates.len(), 1);
        let accepted = validate_boolean_evaluation_candidate(&unit, &candidates[0]).unwrap();
        assert!(matches!(
            accepted.unit().functions[0].blocks[0].nodes[2].operation,
            AbstractOperation::BooleanConstant { value, .. } if value == expected
        ));
    }
}

#[test]
fn integer_range_equality_proves_singleton_outside_and_declines_overlap() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    for kind in [
        IntegerRangeComparisonKind::RangeEqualConstant,
        IntegerRangeComparisonKind::ConstantEqualRange,
    ] {
        assert_eq!(
            evaluate_integer_range_comparison(
                kind,
                scalar_type,
                IntegerValue::Unsigned(7),
                IntegerValue::Unsigned(7),
                IntegerValue::Unsigned(7),
            ),
            Some(true)
        );
        assert_eq!(
            evaluate_integer_range_comparison(
                kind,
                scalar_type,
                IntegerValue::Unsigned(7),
                IntegerValue::Unsigned(9),
                IntegerValue::Unsigned(6),
            ),
            Some(false)
        );
        assert_eq!(
            evaluate_integer_range_comparison(
                kind,
                scalar_type,
                IntegerValue::Unsigned(7),
                IntegerValue::Unsigned(9),
                IntegerValue::Unsigned(10),
            ),
            Some(false)
        );
        assert_eq!(
            evaluate_integer_range_comparison(
                kind,
                scalar_type,
                IntegerValue::Unsigned(7),
                IntegerValue::Unsigned(9),
                IntegerValue::Unsigned(8),
            ),
            None
        );
    }
}

#[test]
fn integer_range_pair_comparisons_prove_only_universal_results() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let u = IntegerValue::Unsigned;
    assert_eq!(
        evaluate_integer_range_pair_comparison(
            IntegerRangePairComparisonKind::Equal,
            scalar_type,
            false,
            u(7),
            u(7),
            u(7),
            u(7),
        ),
        Some(true)
    );
    assert_eq!(
        evaluate_integer_range_pair_comparison(
            IntegerRangePairComparisonKind::Equal,
            scalar_type,
            false,
            u(1),
            u(3),
            u(4),
            u(6),
        ),
        Some(false)
    );
    assert_eq!(
        evaluate_integer_range_pair_comparison(
            IntegerRangePairComparisonKind::Equal,
            scalar_type,
            false,
            u(1),
            u(4),
            u(3),
            u(6),
        ),
        None
    );
    assert_eq!(
        evaluate_integer_range_pair_comparison(
            IntegerRangePairComparisonKind::LessThan,
            scalar_type,
            false,
            u(1),
            u(3),
            u(4),
            u(6),
        ),
        Some(true)
    );
    assert_eq!(
        evaluate_integer_range_pair_comparison(
            IntegerRangePairComparisonKind::LessThan,
            scalar_type,
            false,
            u(4),
            u(6),
            u(1),
            u(4),
        ),
        Some(false)
    );
    assert_eq!(
        evaluate_integer_range_pair_comparison(
            IntegerRangePairComparisonKind::LessOrEqual,
            scalar_type,
            false,
            u(1),
            u(4),
            u(4),
            u(6),
        ),
        Some(true)
    );
    assert_eq!(
        evaluate_integer_range_pair_comparison(
            IntegerRangePairComparisonKind::LessOrEqual,
            scalar_type,
            false,
            u(5),
            u(6),
            u(1),
            u(4),
        ),
        Some(false)
    );
    for (kind, expected) in [
        (IntegerRangePairComparisonKind::Equal, true),
        (IntegerRangePairComparisonKind::LessThan, false),
        (IntegerRangePairComparisonKind::LessOrEqual, true),
    ] {
        assert_eq!(
            evaluate_integer_range_pair_comparison(kind, scalar_type, true, u(1), u(6), u(1), u(6),),
            Some(expected)
        );
    }
}

#[test]
fn proof_derived_range_pair_proposes_two_fact_boolean_fold() {
    let unit = proof_range_pair_comparison_unit();
    let ranges = compute_analysis(&unit, AnalysisKind::ValueRanges).unwrap();
    let candidates = IntegerLessOrEqualRangeRangeRule
        .propose(&unit, RuleAnalysisView::new(&[ranges]))
        .unwrap();
    let [candidate] = candidates.as_slice() else {
        panic!("two applicable proof ranges produce one comparison candidate")
    };
    let (left_range_fact, right_range_fact) = candidate
        .scalar_evaluation_witness()
        .and_then(IntegerEvaluationWitness::range_against_range)
        .expect("range-pair candidate retains both proof facts");
    assert_ne!(left_range_fact, right_range_fact);
    assert_eq!(candidate.consumed_facts().len(), 2);
    assert!(matches!(
        candidate.patch(),
        PsiRewritePatch::ReplaceBooleanOperationWithConstant(BooleanConstantRewrite {
            constant: true,
            ..
        })
    ));
    let accepted = validate_boolean_evaluation_candidate(&unit, candidate).unwrap();
    assert!(matches!(
        accepted.unit().functions[0].blocks[0].nodes[2].operation,
        AbstractOperation::BooleanConstant { value: true, .. }
    ));
}

#[test]
fn sccp_registry_appends_range_pair_comparisons_after_literal_range_rules() {
    let registry =
        registry_for_optimization(Optimization::SparseConditionalConstantPropagation).unwrap();
    let contracts = registry.contracts().collect::<Vec<_>>();
    assert_eq!(contracts.len(), 39);
    assert_eq!(
        contracts[34].identity(),
        IntegerEqualRangeConstantRule::contract().identity()
    );
    assert_eq!(
        contracts[35].identity(),
        IntegerEqualConstantRangeRule::contract().identity()
    );
    assert_eq!(
        contracts[36].identity(),
        IntegerEqualRangeRangeRule::contract().identity()
    );
    assert_eq!(
        contracts[37].identity(),
        IntegerLessThanRangeRangeRule::contract().identity()
    );
    assert_eq!(
        contracts[38].identity(),
        IntegerLessOrEqualRangeRangeRule::contract().identity()
    );
    assert!(contracts.iter().all(|contract| {
        contract.pass() == OptimizationPassIdentity::from_canonical_bytes(SCCP_PASS_NAME)
    }));
}
