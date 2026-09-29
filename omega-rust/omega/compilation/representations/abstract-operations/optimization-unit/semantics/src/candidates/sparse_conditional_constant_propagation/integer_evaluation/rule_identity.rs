//! Independent operation-to-rule identity replay for integer constant evaluation.

use abstract_operations::{
    AbstractOperation as O, IntegerArithmetic, IntegerArithmeticOperator, IntegerOverflowPolicy,
    IntegerShiftDirection, IntegerShiftPolicy,
};
use optimization_core::OptimizationRuleIdentity;

use crate::OptimizationUnitValidationError;

pub(super) fn validate(
    operation: &O,
    actual: OptimizationRuleIdentity,
) -> Result<(), OptimizationUnitValidationError> {
    let canonical_name: &[u8] = match operation {
        O::IntegerArithmetic { arithmetic, .. } => arithmetic_rule_name(*arithmetic),
        O::IntegerShift {
            policy, direction, ..
        } => shift_rule_name(*policy, *direction),
        O::IntegerExactCast { .. } => b"omega.psi-rule.exact-integer-cast-constants.v1",
        O::IntegerWiden { .. } => b"omega.psi-rule.integer-widen-constants.v1",
        O::IntegerBitwiseNot { .. } => b"omega.psi-rule.integer-bitwise-not-constants.v1",
        O::IntegerBitwiseAnd { .. } => b"omega.psi-rule.integer-bitwise-and-constants.v1",
        O::IntegerBitwiseOr { .. } => b"omega.psi-rule.integer-bitwise-or-constants.v1",
        O::IntegerBitwiseXor { .. } => b"omega.psi-rule.integer-bitwise-xor-constants.v1",
        _ => return Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    };
    let expected = OptimizationRuleIdentity::from_canonical_bytes(canonical_name);
    if actual != expected {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    Ok(())
}

/// The stable constant-evaluation rule of each arithmetic policy pair.
fn arithmetic_rule_name(arithmetic: IntegerArithmetic) -> &'static [u8] {
    use IntegerArithmeticOperator as Operator;
    use IntegerOverflowPolicy as Policy;
    match (arithmetic.policy(), arithmetic.operator()) {
        (Policy::Exact, Operator::Add) => b"omega.psi-rule.exact-integer-add-constants.v1",
        (Policy::Exact, Operator::Subtract) => {
            b"omega.psi-rule.exact-integer-subtract-constants.v1"
        }
        (Policy::Exact, Operator::Multiply) => {
            b"omega.psi-rule.exact-integer-multiply-constants.v1"
        }
        (Policy::Exact, Operator::Divide) => b"omega.psi-rule.exact-integer-divide-constants.v1",
        (Policy::Exact, Operator::Remainder) => {
            b"omega.psi-rule.exact-integer-remainder-constants.v1"
        }
        (Policy::Wrapping, Operator::Add) => b"omega.psi-rule.wrapping-integer-add-constants.v1",
        (Policy::Wrapping, Operator::Subtract) => {
            b"omega.psi-rule.wrapping-integer-subtract-constants.v1"
        }
        (Policy::Wrapping, Operator::Multiply) => {
            b"omega.psi-rule.wrapping-integer-multiply-constants.v1"
        }
        (Policy::Wrapping, Operator::Divide) => {
            b"omega.psi-rule.wrapping-integer-divide-constants.v1"
        }
        (Policy::Wrapping, Operator::Remainder) => {
            b"omega.psi-rule.wrapping-integer-remainder-constants.v1"
        }
        (Policy::Saturating, Operator::Add) => {
            b"omega.psi-rule.saturating-integer-add-constants.v1"
        }
        (Policy::Saturating, Operator::Subtract) => {
            b"omega.psi-rule.saturating-integer-subtract-constants.v1"
        }
        (Policy::Saturating, Operator::Multiply) => {
            b"omega.psi-rule.saturating-integer-multiply-constants.v1"
        }
        (Policy::Saturating, Operator::Divide) => {
            b"omega.psi-rule.saturating-integer-divide-constants.v1"
        }
        (Policy::Saturating, Operator::Remainder) => {
            b"omega.psi-rule.saturating-integer-remainder-constants.v1"
        }
    }
}

/// The stable constant-evaluation rule of each shift policy and direction.
fn shift_rule_name(policy: IntegerShiftPolicy, direction: IntegerShiftDirection) -> &'static [u8] {
    match (policy, direction) {
        (IntegerShiftPolicy::Exact { .. }, IntegerShiftDirection::Left) => {
            b"omega.psi-rule.exact-integer-shift-left-constants.v1"
        }
        (IntegerShiftPolicy::Exact { .. }, IntegerShiftDirection::Right) => {
            b"omega.psi-rule.exact-integer-shift-right-constants.v1"
        }
        (IntegerShiftPolicy::Wrapping, IntegerShiftDirection::Left) => {
            b"omega.psi-rule.wrapping-integer-shift-left-constants.v1"
        }
        (IntegerShiftPolicy::Wrapping, IntegerShiftDirection::Right) => {
            b"omega.psi-rule.wrapping-integer-shift-right-constants.v1"
        }
    }
}
