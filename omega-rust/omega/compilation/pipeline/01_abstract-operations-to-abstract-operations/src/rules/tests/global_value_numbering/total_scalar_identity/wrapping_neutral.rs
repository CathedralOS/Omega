//! Obligation-free wrapping identity partitions and canonical tie behavior.
use crate::rules::registry::PsiOptimizationRule;

use super::super::super::super::WrappingNeutralArithmeticIdentityRule;
use crate::rules::tests::fixtures::global_value_numbering::{
    WrappingNeutralOperation, wrapping_neutral_identity_unit,
    wrapping_neutral_identity_unit_with_type_and_liveness,
};
use crate::rules::tests::fixtures::id;
use crate::{AnalysisProduct, RuleAnalysisView};
use abstract_operations::AbstractOperation as O;
use optimization_core::{OptimizationFactReference, OptimizationRuleContract};
use optimization_unit::{
    PsiOptimizationUnit, PsiRewriteCandidate, PsiRewritePatch, TotalScalarIdentityKind,
};
use optimization_unit_semantics::validate_total_scalar_identity_candidate;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, OperationId, ValueId};

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

#[test]
fn rule_accepts_signed_and_wider_unsigned_typed_literals() {
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
fn rule_emits_exactly_the_five_semantic_rows() {
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
