//! Wrapping shift-by-zero-count semantic and replay coverage.

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
    let products = analysis_products(unit, WrappingShiftZeroCountIdentityRule::contract());
    WrappingShiftZeroCountIdentityRule
        .propose(unit, RuleAnalysisView::new(&products))
        .unwrap()
}

#[test]
fn rule_replays_both_directions_and_distinct_count_types() {
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
        let first = candidates(&unit);
        assert_eq!(first, candidates(&unit));
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
fn rule_rejects_nonzero_exact_dead_and_mistyped_counts() {
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
        assert!(candidates(&nonzero).is_empty());
        let dead = wrapping_neutral_identity_unit_with_value_and_identity_types_and_liveness(
            IntegerType::new(IntegerSign::Signed, 16).unwrap(),
            IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
            operation,
            IntegerValue::Unsigned(0),
            false,
            false,
            false,
        );
        assert!(candidates(&dead).is_empty());
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
    assert!(candidates(&exact).is_empty());

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
    assert!(candidates(&mistyped).is_empty());
}
