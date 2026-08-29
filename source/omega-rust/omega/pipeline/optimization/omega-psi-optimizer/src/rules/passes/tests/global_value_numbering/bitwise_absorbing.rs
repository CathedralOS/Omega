//! Exact-width bitwise absorbing-literal and overlap custody.

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

fn absorbing_candidates(unit: &PsiOptimizationUnit) -> Vec<PsiRewriteCandidate> {
    let products = analysis_products(unit, BitwiseAbsorbingLiteralIdentityRule::contract());
    BitwiseAbsorbingLiteralIdentityRule
        .propose(unit, RuleAnalysisView::new(&products))
        .unwrap()
}

fn neutral_candidates(unit: &PsiOptimizationUnit) -> Vec<PsiRewriteCandidate> {
    let products = analysis_products(unit, BitwiseNeutralLiteralIdentityRule::contract());
    BitwiseNeutralLiteralIdentityRule
        .propose(unit, RuleAnalysisView::new(&products))
        .unwrap()
}

#[test]
fn bitwise_absorbing_rule_replays_all_four_rows_and_width_boundaries() {
    for (integer, operation, literal, literal_left, expected) in [
        (
            IntegerType::new(IntegerSign::Unsigned, 1).unwrap(),
            BitwiseNeutralOperation::And,
            IntegerValue::Unsigned(0),
            true,
            TotalScalarIdentityKind::IntegerBitwiseAndZeroLeft,
        ),
        (
            IntegerType::new(IntegerSign::Signed, 1).unwrap(),
            BitwiseNeutralOperation::And,
            IntegerValue::Signed(0),
            false,
            TotalScalarIdentityKind::IntegerBitwiseAndZeroRight,
        ),
        (
            IntegerType::new(IntegerSign::Unsigned, 128).unwrap(),
            BitwiseNeutralOperation::And,
            IntegerValue::Unsigned(0),
            false,
            TotalScalarIdentityKind::IntegerBitwiseAndZeroRight,
        ),
        (
            IntegerType::new(IntegerSign::Signed, 1).unwrap(),
            BitwiseNeutralOperation::Or,
            IntegerValue::Signed(-1),
            true,
            TotalScalarIdentityKind::IntegerBitwiseOrAllOnesLeft,
        ),
        (
            IntegerType::new(IntegerSign::Signed, 128).unwrap(),
            BitwiseNeutralOperation::Or,
            IntegerValue::Signed(-1),
            false,
            TotalScalarIdentityKind::IntegerBitwiseOrAllOnesRight,
        ),
        (
            IntegerType::new(IntegerSign::Unsigned, 128).unwrap(),
            BitwiseNeutralOperation::Or,
            IntegerValue::Unsigned(u128::MAX),
            true,
            TotalScalarIdentityKind::IntegerBitwiseOrAllOnesLeft,
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
        let proposed = absorbing_candidates(&unit);
        assert_eq!(proposed, absorbing_candidates(&unit));
        let [candidate] = proposed.try_into().unwrap();
        let PsiRewritePatch::EliminateTotalScalarIdentity(patch) = candidate.patch() else {
            panic!("bitwise absorbing law must use the total identity patch")
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
                b"omega.validator.live-obligation-free-integer-bitwise-absorbing-literal-elimination.v1",
            )
        );
        assert!(matches!(
            accepted.unit().functions[0].blocks[0].nodes[1].operation,
            O::Return { value, .. } if value == id(1_904, ValueId::new)
        ));
    }
}

#[test]
fn bitwise_neutral_and_absorbing_overlaps_are_confluent() {
    let all_ones = IntegerValue::Unsigned(u8::MAX.into());
    for (operation, left, right, neutral_kind, absorbing_kind, replacement) in [
        (
            BitwiseNeutralOperation::And,
            IntegerValue::Unsigned(0),
            all_ones,
            TotalScalarIdentityKind::IntegerBitwiseAndAllOnesRight,
            TotalScalarIdentityKind::IntegerBitwiseAndZeroLeft,
            id(1_953, ValueId::new),
        ),
        (
            BitwiseNeutralOperation::And,
            all_ones,
            IntegerValue::Unsigned(0),
            TotalScalarIdentityKind::IntegerBitwiseAndAllOnesLeft,
            TotalScalarIdentityKind::IntegerBitwiseAndZeroRight,
            id(1_954, ValueId::new),
        ),
        (
            BitwiseNeutralOperation::Or,
            IntegerValue::Unsigned(0),
            all_ones,
            TotalScalarIdentityKind::IntegerBitwiseOrZeroLeft,
            TotalScalarIdentityKind::IntegerBitwiseOrAllOnesRight,
            id(1_954, ValueId::new),
        ),
        (
            BitwiseNeutralOperation::Or,
            all_ones,
            IntegerValue::Unsigned(0),
            TotalScalarIdentityKind::IntegerBitwiseOrZeroRight,
            TotalScalarIdentityKind::IntegerBitwiseOrAllOnesLeft,
            id(1_953, ValueId::new),
        ),
    ] {
        let unit = bitwise_literal_pair_unit(operation, left, right);
        let [neutral] = neutral_candidates(&unit).try_into().unwrap();
        let [absorbing] = absorbing_candidates(&unit).try_into().unwrap();
        let PsiRewritePatch::EliminateTotalScalarIdentity(neutral_patch) = neutral.patch() else {
            unreachable!()
        };
        let PsiRewritePatch::EliminateTotalScalarIdentity(absorbing_patch) = absorbing.patch()
        else {
            unreachable!()
        };
        assert_eq!(neutral_patch.identity, neutral_kind);
        assert_eq!(absorbing_patch.identity, absorbing_kind);
        assert_eq!(neutral_patch.replacement, replacement);
        assert_eq!(absorbing_patch.replacement, replacement);
        assert_eq!(
            validate_total_scalar_identity_candidate(&unit, &neutral)
                .unwrap()
                .unit(),
            validate_total_scalar_identity_candidate(&unit, &absorbing)
                .unwrap()
                .unit(),
        );
    }
}

#[test]
fn bitwise_absorbing_rule_rejects_wrong_literals_dead_impure_and_other_operations() {
    for (operation, wrong_literal) in [
        (BitwiseNeutralOperation::And, IntegerValue::Unsigned(1)),
        (BitwiseNeutralOperation::Or, IntegerValue::Unsigned(0)),
        (
            BitwiseNeutralOperation::Xor,
            IntegerValue::Unsigned(u8::MAX.into()),
        ),
    ] {
        let unit = bitwise_neutral_identity_unit(operation, wrong_literal, false, false);
        assert!(absorbing_candidates(&unit).is_empty());
    }

    let dead = bitwise_neutral_identity_unit_with_type_and_liveness(
        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        BitwiseNeutralOperation::And,
        IntegerValue::Unsigned(0),
        false,
        false,
        false,
    );
    assert!(absorbing_candidates(&dead).is_empty());

    let unit = bitwise_neutral_identity_unit(
        BitwiseNeutralOperation::And,
        IntegerValue::Unsigned(0),
        false,
        false,
    );
    let mut products = analysis_products(&unit, BitwiseAbsorbingLiteralIdentityRule::contract());
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
        BitwiseAbsorbingLiteralIdentityRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn bitwise_absorbing_ties_choose_left_literal_rows_canonically() {
    for (operation, literal, expected) in [
        (
            BitwiseNeutralOperation::And,
            IntegerValue::Unsigned(0),
            TotalScalarIdentityKind::IntegerBitwiseAndZeroLeft,
        ),
        (
            BitwiseNeutralOperation::Or,
            IntegerValue::Unsigned(u8::MAX.into()),
            TotalScalarIdentityKind::IntegerBitwiseOrAllOnesLeft,
        ),
    ] {
        let unit = bitwise_neutral_identity_unit(operation, literal, true, true);
        let [candidate] = absorbing_candidates(&unit).try_into().unwrap();
        let PsiRewritePatch::EliminateTotalScalarIdentity(patch) = candidate.patch() else {
            unreachable!()
        };
        assert_eq!(patch.identity, expected);
    }
}
