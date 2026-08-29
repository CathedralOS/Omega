//! Exact-width bitwise neutral-literal rule coverage.

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
    let products = analysis_products(unit, BitwiseNeutralLiteralIdentityRule::contract());
    BitwiseNeutralLiteralIdentityRule
        .propose(unit, RuleAnalysisView::new(&products))
        .unwrap()
}

#[test]
fn bitwise_neutral_rule_replays_all_six_rows_and_width_boundaries() {
    for (integer, operation, literal, literal_left, expected) in [
        (
            IntegerType::new(IntegerSign::Unsigned, 1).unwrap(),
            BitwiseNeutralOperation::And,
            IntegerValue::Unsigned(1),
            true,
            TotalScalarIdentityKind::IntegerBitwiseAndAllOnesLeft,
        ),
        (
            IntegerType::new(IntegerSign::Signed, 1).unwrap(),
            BitwiseNeutralOperation::And,
            IntegerValue::Signed(-1),
            false,
            TotalScalarIdentityKind::IntegerBitwiseAndAllOnesRight,
        ),
        (
            IntegerType::new(IntegerSign::Unsigned, 128).unwrap(),
            BitwiseNeutralOperation::And,
            IntegerValue::Unsigned(u128::MAX),
            false,
            TotalScalarIdentityKind::IntegerBitwiseAndAllOnesRight,
        ),
        (
            IntegerType::new(IntegerSign::Signed, 128).unwrap(),
            BitwiseNeutralOperation::And,
            IntegerValue::Signed(-1),
            true,
            TotalScalarIdentityKind::IntegerBitwiseAndAllOnesLeft,
        ),
        (
            IntegerType::new(IntegerSign::Signed, 1).unwrap(),
            BitwiseNeutralOperation::Or,
            IntegerValue::Signed(0),
            true,
            TotalScalarIdentityKind::IntegerBitwiseOrZeroLeft,
        ),
        (
            IntegerType::new(IntegerSign::Unsigned, 128).unwrap(),
            BitwiseNeutralOperation::Or,
            IntegerValue::Unsigned(0),
            false,
            TotalScalarIdentityKind::IntegerBitwiseOrZeroRight,
        ),
        (
            IntegerType::new(IntegerSign::Signed, 128).unwrap(),
            BitwiseNeutralOperation::Xor,
            IntegerValue::Signed(0),
            true,
            TotalScalarIdentityKind::IntegerBitwiseXorZeroLeft,
        ),
        (
            IntegerType::new(IntegerSign::Unsigned, 1).unwrap(),
            BitwiseNeutralOperation::Xor,
            IntegerValue::Unsigned(0),
            false,
            TotalScalarIdentityKind::IntegerBitwiseXorZeroRight,
        ),
    ] {
        let unit = bitwise_neutral_identity_unit_with_type_and_liveness(
            integer,
            operation,
            literal,
            literal_left,
            false,
            true,
        );
        let proposed = candidates(&unit);
        assert_eq!(proposed, candidates(&unit));
        let [candidate] = proposed.try_into().unwrap();
        let PsiRewritePatch::EliminateTotalScalarIdentity(patch) = candidate.patch() else {
            panic!("bitwise neutral law must use the total identity patch")
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
                b"omega.validator.live-obligation-free-integer-bitwise-neutral-literal-elimination.v1",
            )
        );
        assert!(matches!(
            accepted.unit().functions[0].blocks[0].nodes[1].operation,
            O::Return { value, .. } if value == id(1_903, ValueId::new)
        ));
    }
}

#[test]
fn bitwise_neutral_ties_choose_left_literal_rows_canonically() {
    for (operation, literal, expected) in [
        (
            BitwiseNeutralOperation::And,
            IntegerValue::Unsigned(u8::MAX.into()),
            TotalScalarIdentityKind::IntegerBitwiseAndAllOnesLeft,
        ),
        (
            BitwiseNeutralOperation::Or,
            IntegerValue::Unsigned(0),
            TotalScalarIdentityKind::IntegerBitwiseOrZeroLeft,
        ),
        (
            BitwiseNeutralOperation::Xor,
            IntegerValue::Unsigned(0),
            TotalScalarIdentityKind::IntegerBitwiseXorZeroLeft,
        ),
    ] {
        let unit = bitwise_neutral_identity_unit(operation, literal, true, true);
        let [candidate] = candidates(&unit).try_into().unwrap();
        let PsiRewritePatch::EliminateTotalScalarIdentity(patch) = candidate.patch() else {
            unreachable!()
        };
        assert_eq!(patch.identity, expected);
    }
}

#[test]
fn bitwise_neutral_rule_rejects_wrong_literals_dead_impure_and_mistyped_inputs() {
    for (operation, wrong_literal) in [
        (BitwiseNeutralOperation::And, IntegerValue::Unsigned(0)),
        (BitwiseNeutralOperation::Or, IntegerValue::Unsigned(1)),
        (BitwiseNeutralOperation::Xor, IntegerValue::Unsigned(1)),
    ] {
        let unit = bitwise_neutral_identity_unit(operation, wrong_literal, false, false);
        assert!(candidates(&unit).is_empty());
    }

    let dead = bitwise_neutral_identity_unit_with_type_and_liveness(
        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        BitwiseNeutralOperation::Or,
        IntegerValue::Unsigned(0),
        false,
        false,
        false,
    );
    assert!(candidates(&dead).is_empty());

    let unit = bitwise_neutral_identity_unit(
        BitwiseNeutralOperation::Or,
        IntegerValue::Unsigned(0),
        false,
        false,
    );
    let mut products = analysis_products(&unit, BitwiseNeutralLiteralIdentityRule::contract());
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
        BitwiseNeutralLiteralIdentityRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );

    let mut mistyped = unit.clone();
    let O::IntegerBitwiseOr {
        psi_operation,
        result,
        left,
        right,
        ..
    } = mistyped.functions[0].blocks[0].nodes[1].operation
    else {
        unreachable!()
    };
    mistyped.functions[0].blocks[0].nodes[1].operation = O::IntegerBitwiseOr {
        psi_operation,
        result,
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
        left,
        right,
    };
    mistyped.identity = recompute_psi_optimization_unit_identity(&mistyped);
    assert!(candidates(&mistyped).is_empty());

    let wrapping = wrapping_neutral_identity_unit(
        WrappingNeutralOperation::Add,
        IntegerValue::Unsigned(0),
        false,
        false,
    );
    assert!(candidates(&wrapping).is_empty());
}
