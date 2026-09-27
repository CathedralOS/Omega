//! Saturating multiply-zero annihilation and overlap custody.
use crate::rules::registry::PsiOptimizationRule;

use super::super::super::super::SaturatingMultiplyZeroAnnihilationRule;
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
