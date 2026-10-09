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
    match operation {
        O::IntegerShift {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
            policy: IntegerShiftPolicy::Wrapping,
            direction: IntegerShiftDirection::Left,
        } => vec![TotalScalarIdentityShape {
            source_operation: *psi_operation,
            result: *result,
            replacement: *value,
            law_operand: *count,
            scalar_type: *value_type,
            law_operand_type: *count_type,
            identity: TotalScalarIdentityKind::WrappingIntegerShiftLeftZeroCount,
            expected_law_value: count_type.zero(),
        }],
        O::IntegerShift {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
            policy: IntegerShiftPolicy::Wrapping,
            direction: IntegerShiftDirection::Right,
        } => vec![TotalScalarIdentityShape {
            source_operation: *psi_operation,
            result: *result,
            replacement: *value,
            law_operand: *count,
            scalar_type: *value_type,
            law_operand_type: *count_type,
            identity: TotalScalarIdentityKind::WrappingIntegerShiftRightZeroCount,
            expected_law_value: count_type.zero(),
        }],
        _ => Vec::new(),
    }
}
