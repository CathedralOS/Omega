//! Saturating multiply-zero annihilation and overlap custody.

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

fn neutral_candidates(unit: &PsiOptimizationUnit) -> Vec<PsiRewriteCandidate> {
    let products = analysis_products(unit, SaturatingNeutralArithmeticIdentityRule::contract());
    SaturatingNeutralArithmeticIdentityRule
        .propose(unit, RuleAnalysisView::new(&products))
        .unwrap()
}

fn annihilation_candidates(unit: &PsiOptimizationUnit) -> Vec<PsiRewriteCandidate> {
    let products = analysis_products(unit, SaturatingMultiplyZeroAnnihilationRule::contract());
    SaturatingMultiplyZeroAnnihilationRule
        .propose(unit, RuleAnalysisView::new(&products))
        .unwrap()
}

#[test]
fn saturating_multiply_zero_annihilation_replays_both_sides_and_integer_boundaries() {
    for (integer, literal_left, literal, expected) in [
        (
            IntegerType::new(IntegerSign::Unsigned, 1).unwrap(),
            true,
            IntegerValue::Unsigned(0),
            TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroLeft,
        ),
        (
            IntegerType::new(IntegerSign::Signed, 1).unwrap(),
            true,
            IntegerValue::Signed(0),
            TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroLeft,
        ),
        (
            IntegerType::new(IntegerSign::Signed, 17).unwrap(),
            false,
            IntegerValue::Signed(0),
            TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroRight,
        ),
        (
            IntegerType::new(IntegerSign::Signed, 128).unwrap(),
            false,
            IntegerValue::Signed(0),
            TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroRight,
        ),
        (
            IntegerType::new(IntegerSign::Unsigned, 128).unwrap(),
            false,
            IntegerValue::Unsigned(0),
            TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroRight,
        ),
    ] {
        let unit = saturating_neutral_identity_unit_with_type_and_liveness(
            integer,
            SaturatingNeutralOperation::Multiply,
            literal,
            literal_left,
            false,
            true,
        );
        let first = annihilation_candidates(&unit);
        assert_eq!(first, annihilation_candidates(&unit));
        let [candidate] = first.try_into().unwrap();
        let PsiRewritePatch::EliminateTotalScalarIdentity(patch) = candidate.patch() else {
            panic!("saturating multiply zero must use the total identity patch")
        };
        assert_eq!(patch.identity, expected);
        assert_eq!(patch.scalar_type, integer);
        assert_eq!(patch.replacement, id(1_904, ValueId::new));
        assert_eq!(candidate.consumed_facts().len(), 1);
        assert!(candidate.accepted_obligation_witness().is_none());
        let accepted = validate_total_scalar_identity_candidate(&unit, &candidate).unwrap();
        assert_eq!(
            accepted.validator(),
            OptimizationValidatorIdentity::from_canonical_bytes(
                b"omega.validator.live-obligation-free-saturating-integer-multiply-zero-annihilation.v1",
            )
        );
        assert!(matches!(
            accepted.unit().functions[0].blocks[0].nodes[1].operation,
            O::Return { value, .. } if value == id(1_904, ValueId::new)
        ));
    }
}

#[test]
fn saturating_multiply_zero_overlap_is_confluent_across_exact_rules() {
    for (left, right, neutral_kind, annihilation_kind, zero) in [
        (
            IntegerValue::Unsigned(0),
            IntegerValue::Unsigned(1),
            TotalScalarIdentityKind::SaturatingIntegerMultiplyOneRight,
            TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroLeft,
            id(1_953, ValueId::new),
        ),
        (
            IntegerValue::Unsigned(1),
            IntegerValue::Unsigned(0),
            TotalScalarIdentityKind::SaturatingIntegerMultiplyOneLeft,
            TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroRight,
            id(1_954, ValueId::new),
        ),
    ] {
        let unit = saturating_multiply_literal_pair_unit(left, right);
        let [neutral] = neutral_candidates(&unit).try_into().unwrap();
        let [annihilation] = annihilation_candidates(&unit).try_into().unwrap();
        let PsiRewritePatch::EliminateTotalScalarIdentity(neutral_patch) = neutral.patch() else {
            unreachable!()
        };
        let PsiRewritePatch::EliminateTotalScalarIdentity(annihilation_patch) =
            annihilation.patch()
        else {
            unreachable!()
        };
        assert_eq!(neutral_patch.identity, neutral_kind);
        assert_eq!(annihilation_patch.identity, annihilation_kind);
        assert_eq!(neutral_patch.replacement, zero);
        assert_eq!(annihilation_patch.replacement, zero);
        assert_eq!(
            validate_total_scalar_identity_candidate(&unit, &neutral)
                .unwrap()
                .unit(),
            validate_total_scalar_identity_candidate(&unit, &annihilation)
                .unwrap()
                .unit(),
        );
    }
}

#[test]
fn saturating_multiply_zero_annihilation_rejects_nonzero_dead_impure_and_other_policies() {
    for literal in [IntegerValue::Unsigned(1), IntegerValue::Unsigned(2)] {
        let unit = saturating_neutral_identity_unit(
            SaturatingNeutralOperation::Multiply,
            literal,
            false,
            false,
        );
        assert!(annihilation_candidates(&unit).is_empty());
    }

    let dead = saturating_neutral_identity_unit_with_type_and_liveness(
        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        SaturatingNeutralOperation::Multiply,
        IntegerValue::Unsigned(0),
        false,
        false,
        false,
    );
    assert!(annihilation_candidates(&dead).is_empty());

    let unit = saturating_neutral_identity_unit(
        SaturatingNeutralOperation::Multiply,
        IntegerValue::Unsigned(0),
        false,
        false,
    );
    let mut products = analysis_products(&unit, SaturatingMultiplyZeroAnnihilationRule::contract());
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
        SaturatingMultiplyZeroAnnihilationRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );

    let mut exact = unit.clone();
    exact.functions[0].blocks[0].nodes[1].operation = O::ExactIntegerMultiply {
        psi_operation: id(1_907, OperationId::new),
        obligation: id(1_909, ObligationId::new),
        result: id(1_905, ValueId::new),
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        left: id(1_903, ValueId::new),
        right: id(1_904, ValueId::new),
    };
    exact.identity = recompute_psi_optimization_unit_identity(&exact);
    assert!(annihilation_candidates(&exact).is_empty());

    let mut mistyped = saturating_neutral_identity_unit_with_type_and_liveness(
        IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
        SaturatingNeutralOperation::Multiply,
        IntegerValue::Unsigned(0),
        false,
        false,
        true,
    );
    let O::SaturatingIntegerMultiply {
        psi_operation,
        result,
        left,
        right,
        ..
    } = mistyped.functions[0].blocks[0].nodes[1].operation
    else {
        unreachable!()
    };
    mistyped.functions[0].blocks[0].nodes[1].operation = O::SaturatingIntegerMultiply {
        psi_operation,
        result,
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        left,
        right,
    };
    mistyped.identity = recompute_psi_optimization_unit_identity(&mistyped);
    assert!(annihilation_candidates(&mistyped).is_empty());

    let mut wrapping = unit;
    wrapping.functions[0].blocks[0].nodes[1].operation = O::WrappingIntegerMultiply {
        psi_operation: id(1_907, OperationId::new),
        result: id(1_905, ValueId::new),
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        left: id(1_903, ValueId::new),
        right: id(1_904, ValueId::new),
    };
    wrapping.identity = recompute_psi_optimization_unit_identity(&wrapping);
    assert!(annihilation_candidates(&wrapping).is_empty());
}

#[test]
fn saturating_multiply_zero_ties_choose_the_left_zero_canonically() {
    let unit = saturating_neutral_identity_unit(
        SaturatingNeutralOperation::Multiply,
        IntegerValue::Unsigned(0),
        true,
        true,
    );
    let [candidate] = annihilation_candidates(&unit).try_into().unwrap();
    let PsiRewritePatch::EliminateTotalScalarIdentity(patch) = candidate.patch() else {
        unreachable!()
    };
    assert_eq!(
        patch.identity,
        TotalScalarIdentityKind::SaturatingIntegerMultiplyZeroLeft
    );
    assert_eq!(patch.replacement, id(1_904, ValueId::new));
}
