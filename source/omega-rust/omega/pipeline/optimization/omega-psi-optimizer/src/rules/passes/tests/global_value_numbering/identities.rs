//! Obligation-free wrapping identity partitions and canonical tie behavior.

use super::*;

fn analysis_products(
    unit: &PsiOptimizationUnit,
    contract: OptimizationRuleContract,
) -> Vec<AnalysisProduct> {
    let mut manager = crate::AnalysisManager::new(unit);
    manager
        .require_all(unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect()
}

fn candidates(unit: &PsiOptimizationUnit) -> Vec<PsiRewriteCandidate> {
    let products = analysis_products(unit, WrappingNeutralArithmeticIdentityRule::contract());
    WrappingNeutralArithmeticIdentityRule
        .propose(unit, RuleAnalysisView::new(&products))
        .unwrap()
}

fn shift_candidates(unit: &PsiOptimizationUnit) -> Vec<PsiRewriteCandidate> {
    let products = analysis_products(unit, WrappingShiftZeroCountIdentityRule::contract());
    WrappingShiftZeroCountIdentityRule
        .propose(unit, RuleAnalysisView::new(&products))
        .unwrap()
}

#[test]
fn wrapping_neutral_rule_accepts_signed_and_wider_unsigned_typed_literals() {
    for (integer, operation, literal, expected) in [
        (
            IntegerType::new(IntegerSign::Signed, 16).unwrap(),
            WrappingNeutralOperation::Add,
            IntegerValue::Signed(0),
            TotalScalarIdentityKind::WrappingIntegerAddZeroRight,
        ),
        (
            IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
            WrappingNeutralOperation::Multiply,
            IntegerValue::Unsigned(1),
            TotalScalarIdentityKind::WrappingIntegerMultiplyOneLeft,
        ),
    ] {
        let unit = wrapping_neutral_identity_unit_with_type_and_liveness(
            integer,
            operation,
            literal,
            operation == WrappingNeutralOperation::Multiply,
            false,
            true,
        );
        let [candidate] = candidates(&unit).try_into().unwrap();
        let PsiRewritePatch::EliminateTotalScalarIdentity(patch) = candidate.patch() else {
            unreachable!()
        };
        assert_eq!(patch.identity, expected);
        assert_eq!(patch.scalar_type, integer);
        validate_total_scalar_identity_candidate(&unit, &candidate).unwrap();
    }
}

#[test]
fn wrapping_identity_rules_are_disabled_by_default_and_cataloged_once() {
    assert!(
        built_in_psi_registry(&OptimizationSelections::default())
            .unwrap()
            .is_empty()
    );
    let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let identities = registry
        .contracts()
        .map(|contract| contract.identity())
        .collect::<Vec<_>>();
    let contract = WrappingNeutralArithmeticIdentityRule::contract();
    assert_eq!(
        contract.pass(),
        OptimizationPassIdentity::from_canonical_bytes(
            b"omega.psi-pass.global-value-numbering.v10",
        )
    );
    let expected = contract.identity();
    assert_eq!(identities.get(9), Some(&expected));
    assert_eq!(
        identities
            .into_iter()
            .filter(|identity| *identity == expected)
            .count(),
        1
    );
    let shift = WrappingShiftZeroCountIdentityRule::contract().identity();
    assert_eq!(
        registry.contracts().nth(10).map(|row| row.identity()),
        Some(shift)
    );
    assert_eq!(
        registry
            .contracts()
            .filter(|contract| contract.identity() == shift)
            .count(),
        1
    );
    let annihilation = WrappingMultiplyZeroAnnihilationRule::contract().identity();
    assert_eq!(
        registry.contracts().nth(11).map(|row| row.identity()),
        Some(annihilation)
    );
    assert_eq!(
        registry
            .contracts()
            .filter(|contract| contract.identity() == annihilation)
            .count(),
        1
    );
}

#[test]
fn wrapping_shift_zero_count_rule_replays_both_directions_and_distinct_count_types() {
    for (operation, value_type, count_type, literal, expected) in [
        (
            WrappingNeutralOperation::ShiftLeft,
            IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
            IntegerType::new(IntegerSign::Signed, 8).unwrap(),
            IntegerValue::Signed(0),
            TotalScalarIdentityKind::WrappingIntegerShiftLeftZeroCount,
        ),
        (
            WrappingNeutralOperation::ShiftRight,
            IntegerType::new(IntegerSign::Signed, 17).unwrap(),
            IntegerType::new(IntegerSign::Unsigned, 128).unwrap(),
            IntegerValue::Unsigned(0),
            TotalScalarIdentityKind::WrappingIntegerShiftRightZeroCount,
        ),
        (
            WrappingNeutralOperation::ShiftLeft,
            IntegerType::new(IntegerSign::Unsigned, 1).unwrap(),
            IntegerType::new(IntegerSign::Unsigned, 1).unwrap(),
            IntegerValue::Unsigned(0),
            TotalScalarIdentityKind::WrappingIntegerShiftLeftZeroCount,
        ),
    ] {
        let unit = wrapping_neutral_identity_unit_with_value_and_identity_types_and_liveness(
            value_type, count_type, operation, literal, false, false, true,
        );
        let first = shift_candidates(&unit);
        assert_eq!(first, shift_candidates(&unit));
        let [candidate] = first.try_into().unwrap();
        let PsiRewritePatch::EliminateTotalScalarIdentity(patch) = candidate.patch() else {
            panic!("wrapping shift zero count must use the total identity patch")
        };
        assert_eq!(patch.identity, expected);
        assert_eq!(patch.scalar_type, value_type);
        assert_eq!(patch.replacement, id(1_903, ValueId::new));
        assert_eq!(candidate.consumed_facts().len(), 1);
        assert!(candidate.accepted_obligation_witness().is_none());
        let accepted = validate_total_scalar_identity_candidate(&unit, &candidate).unwrap();
        assert_eq!(
            accepted.validator(),
            OptimizationValidatorIdentity::from_canonical_bytes(
                b"omega.validator.live-obligation-free-wrapping-integer-shift-zero-count-elimination.v1",
            )
        );
        assert!(matches!(
            accepted.unit().functions[0].blocks[0].nodes[1].operation,
            O::Return { value, .. } if value == id(1_903, ValueId::new)
        ));
    }
}

#[test]
fn wrapping_shift_zero_count_rule_rejects_nonzero_exact_dead_and_mistyped_counts() {
    for operation in [
        WrappingNeutralOperation::ShiftLeft,
        WrappingNeutralOperation::ShiftRight,
    ] {
        let nonzero = wrapping_neutral_identity_unit_with_value_and_identity_types_and_liveness(
            IntegerType::new(IntegerSign::Signed, 16).unwrap(),
            IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            operation,
            IntegerValue::Unsigned(1),
            false,
            false,
            true,
        );
        assert!(shift_candidates(&nonzero).is_empty());
        let dead = wrapping_neutral_identity_unit_with_value_and_identity_types_and_liveness(
            IntegerType::new(IntegerSign::Signed, 16).unwrap(),
            IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            operation,
            IntegerValue::Unsigned(0),
            false,
            false,
            false,
        );
        assert!(shift_candidates(&dead).is_empty());
    }

    let mut exact = wrapping_neutral_identity_unit_with_value_and_identity_types_and_liveness(
        IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
        IntegerType::new(IntegerSign::Signed, 8).unwrap(),
        WrappingNeutralOperation::ShiftLeft,
        IntegerValue::Signed(0),
        false,
        false,
        true,
    );
    exact.functions[0].blocks[0].nodes[1].operation = O::ExactIntegerShiftLeft {
        psi_operation: id(1_907, OperationId::new),
        obligation: id(1_909, ObligationId::new),
        result: id(1_905, ValueId::new),
        value_type: IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
        count_type: IntegerType::new(IntegerSign::Signed, 8).unwrap(),
        value: id(1_903, ValueId::new),
        count: id(1_904, ValueId::new),
    };
    exact.identity = recompute_psi_optimization_unit_identity(&exact);
    assert!(shift_candidates(&exact).is_empty());

    let impure = wrapping_neutral_identity_unit_with_value_and_identity_types_and_liveness(
        IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
        IntegerType::new(IntegerSign::Signed, 8).unwrap(),
        WrappingNeutralOperation::ShiftLeft,
        IntegerValue::Signed(0),
        false,
        false,
        true,
    );
    let mut products = analysis_products(&impure, WrappingShiftZeroCountIdentityRule::contract());
    let effects = products
        .iter_mut()
        .find_map(|product| match product {
            AnalysisProduct::EffectSummaries(effects) => Some(effects),
            _ => None,
        })
        .unwrap();
    effects
        .nodes
        .iter_mut()
        .find(|row| row.node == 1)
        .unwrap()
        .class = crate::EffectClass::Control;
    assert!(
        WrappingShiftZeroCountIdentityRule
            .propose(&impure, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );

    let mut mistyped = wrapping_neutral_identity_unit_with_value_and_identity_types_and_liveness(
        IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
        IntegerType::new(IntegerSign::Signed, 8).unwrap(),
        WrappingNeutralOperation::ShiftRight,
        IntegerValue::Signed(0),
        false,
        false,
        true,
    );
    let O::WrappingIntegerShiftRight { count_type, .. } =
        &mut mistyped.functions[0].blocks[0].nodes[1].operation
    else {
        unreachable!()
    };
    *count_type = IntegerType::new(IntegerSign::Signed, 16).unwrap();
    mistyped.identity = recompute_psi_optimization_unit_identity(&mistyped);
    assert!(shift_candidates(&mistyped).is_empty());
}

#[test]
fn wrapping_neutral_rule_emits_exactly_the_five_semantic_rows() {
    let cases = [
        (
            WrappingNeutralOperation::Add,
            IntegerValue::Unsigned(0),
            true,
            TotalScalarIdentityKind::WrappingIntegerAddZeroLeft,
        ),
        (
            WrappingNeutralOperation::Add,
            IntegerValue::Unsigned(0),
            false,
            TotalScalarIdentityKind::WrappingIntegerAddZeroRight,
        ),
        (
            WrappingNeutralOperation::Subtract,
            IntegerValue::Unsigned(0),
            false,
            TotalScalarIdentityKind::WrappingIntegerSubtractZeroRight,
        ),
        (
            WrappingNeutralOperation::Multiply,
            IntegerValue::Unsigned(1),
            true,
            TotalScalarIdentityKind::WrappingIntegerMultiplyOneLeft,
        ),
        (
            WrappingNeutralOperation::Multiply,
            IntegerValue::Unsigned(1),
            false,
            TotalScalarIdentityKind::WrappingIntegerMultiplyOneRight,
        ),
    ];

    for (operation, literal, literal_left, identity) in cases {
        let unit = wrapping_neutral_identity_unit(operation, literal, literal_left, false);
        let [candidate] = candidates(&unit).try_into().unwrap();
        let PsiRewritePatch::EliminateTotalScalarIdentity(patch) = candidate.patch() else {
            panic!("wrapping neutral arithmetic must use its dedicated total identity patch")
        };
        assert_eq!(patch.identity, identity);
        assert_eq!(patch.source_operation, id(1_907, OperationId::new));
        assert_eq!(patch.result, id(1_905, ValueId::new));
        assert_eq!(patch.replacement, id(1_903, ValueId::new));
        let constant_fact = candidate
            .total_scalar_identity_witness()
            .expect("the exact literal fact is retained as the sole witness");
        assert_eq!(
            candidate.consumed_facts(),
            [OptimizationFactReference::ScalarConstant(constant_fact)]
        );
        let accepted = validate_total_scalar_identity_candidate(&unit, &candidate).unwrap();
        assert_eq!(accepted.unit().functions[0].blocks[0].nodes.len(), 2);
        assert!(matches!(
            accepted.unit().functions[0].blocks[0].nodes[1].operation,
            O::Return { value, .. } if value == id(1_903, ValueId::new)
        ));
    }
}

#[test]
fn wrapping_neutral_rule_rejects_adjacent_but_unequal_partitions() {
    for unit in [
        wrapping_neutral_identity_unit(
            WrappingNeutralOperation::Add,
            IntegerValue::Unsigned(1),
            true,
            false,
        ),
        wrapping_neutral_identity_unit(
            WrappingNeutralOperation::Subtract,
            IntegerValue::Unsigned(0),
            true,
            false,
        ),
        wrapping_neutral_identity_unit(
            WrappingNeutralOperation::Multiply,
            IntegerValue::Unsigned(0),
            false,
            false,
        ),
    ] {
        assert!(candidates(&unit).is_empty());
    }

    let mut exact_policy = wrapping_neutral_identity_unit(
        WrappingNeutralOperation::Add,
        IntegerValue::Unsigned(0),
        false,
        false,
    );
    exact_policy.functions[0].blocks[0].nodes[1].operation = O::SaturatingIntegerAdd {
        psi_operation: id(1_907, OperationId::new),
        result: id(1_905, ValueId::new),
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        left: id(1_903, ValueId::new),
        right: id(1_904, ValueId::new),
    };
    exact_policy.identity = recompute_psi_optimization_unit_identity(&exact_policy);
    validate_psi_optimization_unit(&exact_policy).unwrap();
    assert!(candidates(&exact_policy).is_empty());
}

#[test]
fn wrapping_neutral_rule_rejects_dead_non_pure_and_type_mismatched_inputs() {
    let dead = wrapping_neutral_identity_unit_with_type_and_liveness(
        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        WrappingNeutralOperation::Add,
        IntegerValue::Unsigned(0),
        false,
        false,
        false,
    );
    assert!(candidates(&dead).is_empty());

    let unit = wrapping_neutral_identity_unit(
        WrappingNeutralOperation::Add,
        IntegerValue::Unsigned(0),
        false,
        false,
    );
    let mut products = analysis_products(&unit, WrappingNeutralArithmeticIdentityRule::contract());
    let effects = products
        .iter_mut()
        .find_map(|product| match product {
            AnalysisProduct::EffectSummaries(effects) => Some(effects),
            _ => None,
        })
        .unwrap();
    effects
        .nodes
        .iter_mut()
        .find(|row| row.node == 1)
        .unwrap()
        .class = crate::EffectClass::Control;
    assert!(
        WrappingNeutralArithmeticIdentityRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );

    let mut mismatched_type = wrapping_neutral_identity_unit_with_type_and_liveness(
        IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
        WrappingNeutralOperation::Add,
        IntegerValue::Unsigned(0),
        false,
        false,
        true,
    );
    let O::WrappingIntegerAdd {
        psi_operation,
        result,
        left,
        right,
        ..
    } = mismatched_type.functions[0].blocks[0].nodes[1].operation
    else {
        unreachable!()
    };
    mismatched_type.functions[0].blocks[0].nodes[1].operation = O::WrappingIntegerAdd {
        psi_operation,
        result,
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        left,
        right,
    };
    mismatched_type.identity = recompute_psi_optimization_unit_identity(&mismatched_type);
    assert!(candidates(&mismatched_type).is_empty());
}

#[test]
fn wrapping_neutral_ties_choose_the_left_identity_row_canonically() {
    for (operation, literal, expected) in [
        (
            WrappingNeutralOperation::Add,
            IntegerValue::Unsigned(0),
            TotalScalarIdentityKind::WrappingIntegerAddZeroLeft,
        ),
        (
            WrappingNeutralOperation::Multiply,
            IntegerValue::Unsigned(1),
            TotalScalarIdentityKind::WrappingIntegerMultiplyOneLeft,
        ),
    ] {
        let unit = wrapping_neutral_identity_unit(operation, literal, true, true);
        let first = candidates(&unit);
        let second = candidates(&unit);
        assert_eq!(first, second);
        let [candidate] = first.try_into().unwrap();
        let PsiRewritePatch::EliminateTotalScalarIdentity(patch) = candidate.patch() else {
            unreachable!()
        };
        assert_eq!(patch.identity, expected);
        assert_eq!(patch.replacement, id(1_904, ValueId::new));
    }
}
