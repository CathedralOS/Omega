//! Saturating neutral-arithmetic identity custody.

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
    let products = analysis_products(unit, SaturatingNeutralArithmeticIdentityRule::contract());
    SaturatingNeutralArithmeticIdentityRule
        .propose(unit, RuleAnalysisView::new(&products))
        .unwrap()
}

#[test]
fn saturating_neutral_rule_replays_all_five_rows_and_integer_boundaries() {
    for (integer, operation, literal, literal_left, expected) in [
        (
            IntegerType::new(IntegerSign::Signed, 1).unwrap(),
            SaturatingNeutralOperation::Add,
            IntegerValue::Signed(0),
            true,
            TotalScalarIdentityKind::SaturatingIntegerAddZeroLeft,
        ),
        (
            IntegerType::new(IntegerSign::Unsigned, 128).unwrap(),
            SaturatingNeutralOperation::Add,
            IntegerValue::Unsigned(0),
            false,
            TotalScalarIdentityKind::SaturatingIntegerAddZeroRight,
        ),
        (
            IntegerType::new(IntegerSign::Signed, 128).unwrap(),
            SaturatingNeutralOperation::Subtract,
            IntegerValue::Signed(0),
            false,
            TotalScalarIdentityKind::SaturatingIntegerSubtractZeroRight,
        ),
        (
            IntegerType::new(IntegerSign::Unsigned, 1).unwrap(),
            SaturatingNeutralOperation::Multiply,
            IntegerValue::Unsigned(1),
            true,
            TotalScalarIdentityKind::SaturatingIntegerMultiplyOneLeft,
        ),
        (
            IntegerType::new(IntegerSign::Signed, 2).unwrap(),
            SaturatingNeutralOperation::Multiply,
            IntegerValue::Signed(1),
            false,
            TotalScalarIdentityKind::SaturatingIntegerMultiplyOneRight,
        ),
    ] {
        let unit = saturating_neutral_identity_unit_with_type_and_liveness(
            integer,
            operation,
            literal,
            literal_left,
            false,
            true,
        );
        let first = candidates(&unit);
        assert_eq!(first, candidates(&unit));
        let [candidate] = first.try_into().unwrap();
        let PsiRewritePatch::EliminateTotalScalarIdentity(patch) = candidate.patch() else {
            panic!("saturating neutral arithmetic must use the total identity patch")
        };
        assert_eq!(patch.identity, expected);
        assert_eq!(patch.scalar_type, integer);
        assert_eq!(patch.replacement, id(1_903, ValueId::new));
        assert_eq!(candidate.consumed_facts().len(), 1);
        assert!(candidate.accepted_obligation_witness().is_none());
        let accepted = validate_total_scalar_identity_candidate(&unit, &candidate).unwrap();
        assert_eq!(
            accepted.validator(),
            OptimizationValidatorIdentity::from_canonical_bytes(
                b"omega.validator.live-obligation-free-saturating-integer-neutral-arithmetic-identity-elimination.v1",
            )
        );
        assert!(matches!(
            accepted.unit().functions[0].blocks[0].nodes[1].operation,
            O::Return { value, .. } if value == id(1_903, ValueId::new)
        ));
    }
}

#[test]
fn saturating_neutral_ties_choose_left_rows_canonically() {
    for (operation, literal, expected) in [
        (
            SaturatingNeutralOperation::Add,
            IntegerValue::Unsigned(0),
            TotalScalarIdentityKind::SaturatingIntegerAddZeroLeft,
        ),
        (
            SaturatingNeutralOperation::Multiply,
            IntegerValue::Unsigned(1),
            TotalScalarIdentityKind::SaturatingIntegerMultiplyOneLeft,
        ),
    ] {
        let unit = saturating_neutral_identity_unit(operation, literal, true, true);
        let [candidate] = candidates(&unit).try_into().unwrap();
        let PsiRewritePatch::EliminateTotalScalarIdentity(patch) = candidate.patch() else {
            unreachable!()
        };
        assert_eq!(patch.identity, expected);
        assert_eq!(patch.replacement, id(1_904, ValueId::new));
    }
}

#[test]
fn saturating_neutral_rule_rejects_non_neutral_dead_impure_and_other_policies() {
    for (operation, literal, literal_left) in [
        (
            SaturatingNeutralOperation::Add,
            IntegerValue::Unsigned(1),
            false,
        ),
        (
            SaturatingNeutralOperation::Subtract,
            IntegerValue::Unsigned(1),
            false,
        ),
        (
            SaturatingNeutralOperation::Subtract,
            IntegerValue::Unsigned(0),
            true,
        ),
        (
            SaturatingNeutralOperation::Multiply,
            IntegerValue::Unsigned(0),
            false,
        ),
    ] {
        let unit = saturating_neutral_identity_unit(operation, literal, literal_left, false);
        assert!(candidates(&unit).is_empty());
    }

    let signed_one_bit = IntegerType::new(IntegerSign::Signed, 1).unwrap();
    assert!(!signed_one_bit.admits(IntegerValue::Signed(1)));
    let signed_one_bit_multiply = saturating_neutral_identity_unit_with_type_and_liveness(
        signed_one_bit,
        SaturatingNeutralOperation::Multiply,
        IntegerValue::Signed(0),
        false,
        false,
        true,
    );
    assert!(candidates(&signed_one_bit_multiply).is_empty());

    let dead = saturating_neutral_identity_unit_with_type_and_liveness(
        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        SaturatingNeutralOperation::Add,
        IntegerValue::Unsigned(0),
        false,
        false,
        false,
    );
    assert!(candidates(&dead).is_empty());

    let unit = saturating_neutral_identity_unit(
        SaturatingNeutralOperation::Add,
        IntegerValue::Unsigned(0),
        false,
        false,
    );
    let mut products =
        analysis_products(&unit, SaturatingNeutralArithmeticIdentityRule::contract());
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
        SaturatingNeutralArithmeticIdentityRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );

    let mut wrapping = unit.clone();
    let O::SaturatingIntegerAdd {
        psi_operation,
        result,
        scalar_type,
        left,
        right,
    } = wrapping.functions[0].blocks[0].nodes[1].operation
    else {
        unreachable!()
    };
    wrapping.functions[0].blocks[0].nodes[1].operation = O::WrappingIntegerAdd {
        psi_operation,
        result,
        scalar_type,
        left,
        right,
    };
    wrapping.identity = recompute_psi_optimization_unit_identity(&wrapping);
    assert!(candidates(&wrapping).is_empty());

    let mut exact = unit;
    exact.functions[0].blocks[0].nodes[1].operation = O::ExactIntegerAdd {
        psi_operation,
        obligation: id(1_909, ObligationId::new),
        result,
        scalar_type,
        left,
        right,
    };
    exact.identity = recompute_psi_optimization_unit_identity(&exact);
    assert!(candidates(&exact).is_empty());
}

#[test]
fn saturating_neutral_rule_rejects_type_mismatch() {
    let mut unit = saturating_neutral_identity_unit_with_type_and_liveness(
        IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
        SaturatingNeutralOperation::Add,
        IntegerValue::Unsigned(0),
        false,
        false,
        true,
    );
    let O::SaturatingIntegerAdd {
        psi_operation,
        result,
        left,
        right,
        ..
    } = unit.functions[0].blocks[0].nodes[1].operation
    else {
        unreachable!()
    };
    unit.functions[0].blocks[0].nodes[1].operation = O::SaturatingIntegerAdd {
        psi_operation,
        result,
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        left,
        right,
    };
    unit.identity = recompute_psi_optimization_unit_identity(&unit);
    assert!(candidates(&unit).is_empty());
}
