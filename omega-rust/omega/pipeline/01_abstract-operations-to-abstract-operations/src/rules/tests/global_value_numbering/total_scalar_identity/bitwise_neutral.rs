//! Exact-width bitwise neutral-literal rule coverage.
use crate::rules::registry::PsiOptimizationRule;

use super::super::super::super::BitwiseNeutralLiteralIdentityRule;
use crate::rules::tests::fixtures::global_value_numbering::{
    BitwiseNeutralOperation, bitwise_neutral_identity_unit_with_type_and_liveness,
};
use crate::rules::tests::fixtures::id;
use crate::{AnalysisProduct, RuleAnalysisView};
use abstract_operations::AbstractOperation as O;
use optimization_core::{OptimizationRuleContract, OptimizationValidatorIdentity};
use optimization_unit::{
    PsiOptimizationUnit, PsiRewriteCandidate, PsiRewritePatch, TotalScalarIdentityKind,
};
use optimization_unit_semantics::validate_total_scalar_identity_candidate;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ValueId};

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
