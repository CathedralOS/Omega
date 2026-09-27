//! Saturating neutral-arithmetic identity custody.
use crate::rules::registry::PsiOptimizationRule;

use super::super::super::super::SaturatingNeutralArithmeticIdentityRule;
use crate::rules::tests::fixtures::global_value_numbering::{
    SaturatingNeutralOperation, saturating_neutral_identity_unit_with_type_and_liveness,
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
