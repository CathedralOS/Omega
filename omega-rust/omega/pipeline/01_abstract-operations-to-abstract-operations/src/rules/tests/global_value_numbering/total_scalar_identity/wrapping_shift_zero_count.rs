//! Wrapping shift-by-zero-count semantic and replay coverage.
use crate::rules::registry::PsiOptimizationRule;

use super::super::super::super::WrappingShiftZeroCountIdentityRule;
use crate::rules::tests::fixtures::global_value_numbering::{
    WrappingNeutralOperation,
    wrapping_neutral_identity_unit_with_value_and_identity_types_and_liveness,
};
use crate::rules::tests::fixtures::id;
use crate::{AnalysisProduct, RuleAnalysisView};
use abstract_operations::AbstractOperation as O;
use optimization_core::OptimizationRuleContract;
use optimization_unit::{
    PsiOptimizationUnit, PsiRewriteCandidate, recompute_psi_optimization_unit_identity,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, ObligationId, OperationId, ValueId,
};

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
