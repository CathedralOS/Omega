//! The exact wrapping shift-zero-count rule, and the closed semantic partition
//! (`classify`) it proposes over.

use abstract_operations::{AbstractOperation as O, IntegerShiftDirection, IntegerShiftPolicy};
use optimization_core::OptimizationRuleContract;
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate, TotalScalarIdentityKind};

use crate::rules::global_value_numbering::total_scalar_identities::{
    TotalScalarIdentityShape, exact_total_scalar_identity, propose_total_scalar_identities,
};
use crate::{PsiOptimizationRule, RuleAnalysisView, RuleProposalError};

#[derive(Debug, Clone, Copy, Default)]
pub struct WrappingShiftZeroCountIdentityRule;

impl WrappingShiftZeroCountIdentityRule {
    pub fn contract() -> OptimizationRuleContract {
        exact_total_scalar_identity(
            b"omega.psi-rule.live-obligation-free-wrapping-integer-shift-zero-count-elimination.v1",
        )
    }
}

psi_optimization_rule_delegate!(
    WrappingShiftZeroCountIdentityRule,
    propose = propose_total_scalar_identities,
    contract = Self::contract(),
    classify
);

pub(super) fn classify(operation: &O) -> Vec<TotalScalarIdentityShape> {
    let Some(operands) = operation.integer_shift() else {
        return Vec::new();
    };
    if !matches!(operands.policy, IntegerShiftPolicy::Wrapping) {
        return Vec::new();
    }
    let identity = match operands.direction {
        IntegerShiftDirection::Left => TotalScalarIdentityKind::WrappingIntegerShiftLeftZeroCount,
        IntegerShiftDirection::Right => TotalScalarIdentityKind::WrappingIntegerShiftRightZeroCount,
    };
    vec![TotalScalarIdentityShape {
        source_operation: operands.psi_operation,
        result: operands.result,
        replacement: operands.value,
        law_operand: operands.count,
        scalar_type: operands.value_type,
        law_operand_type: operands.count_type,
        identity,
        expected_law_value: operands.count_type.zero(),
    }]
}
