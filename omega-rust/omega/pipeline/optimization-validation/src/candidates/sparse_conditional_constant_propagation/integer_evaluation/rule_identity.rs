//! Independent operation-to-rule identity replay for integer constant evaluation.

use abstract_operations::AbstractOperation as O;
use optimization_core::OptimizationRuleIdentity;

use crate::OptimizationUnitValidationError;

pub(super) fn validate(
    operation: &O,
    actual: OptimizationRuleIdentity,
) -> Result<(), OptimizationUnitValidationError> {
    let canonical_name: &[u8] = match operation {
        O::ExactIntegerAdd { .. } => b"omega.psi-rule.exact-integer-add-constants.v1",
        O::ExactIntegerSubtract { .. } => b"omega.psi-rule.exact-integer-subtract-constants.v1",
        O::ExactIntegerMultiply { .. } => b"omega.psi-rule.exact-integer-multiply-constants.v1",
        O::WrappingIntegerAdd { .. } => b"omega.psi-rule.wrapping-integer-add-constants.v1",
        O::WrappingIntegerSubtract { .. } => {
            b"omega.psi-rule.wrapping-integer-subtract-constants.v1"
        }
        O::WrappingIntegerMultiply { .. } => {
            b"omega.psi-rule.wrapping-integer-multiply-constants.v1"
        }
        O::SaturatingIntegerAdd { .. } => b"omega.psi-rule.saturating-integer-add-constants.v1",
        O::SaturatingIntegerSubtract { .. } => {
            b"omega.psi-rule.saturating-integer-subtract-constants.v1"
        }
        O::SaturatingIntegerMultiply { .. } => {
            b"omega.psi-rule.saturating-integer-multiply-constants.v1"
        }
        O::ExactIntegerDivide { .. } => b"omega.psi-rule.exact-integer-divide-constants.v1",
        O::ExactIntegerRemainder { .. } => b"omega.psi-rule.exact-integer-remainder-constants.v1",
        O::WrappingIntegerDivide { .. } => b"omega.psi-rule.wrapping-integer-divide-constants.v1",
        O::WrappingIntegerRemainder { .. } => {
            b"omega.psi-rule.wrapping-integer-remainder-constants.v1"
        }
        O::SaturatingIntegerDivide { .. } => {
            b"omega.psi-rule.saturating-integer-divide-constants.v1"
        }
        O::SaturatingIntegerRemainder { .. } => {
            b"omega.psi-rule.saturating-integer-remainder-constants.v1"
        }
        O::ExactIntegerShiftLeft { .. } => b"omega.psi-rule.exact-integer-shift-left-constants.v1",
        O::ExactIntegerShiftRight { .. } => {
            b"omega.psi-rule.exact-integer-shift-right-constants.v1"
        }
        O::WrappingIntegerShiftLeft { .. } => {
            b"omega.psi-rule.wrapping-integer-shift-left-constants.v1"
        }
        O::WrappingIntegerShiftRight { .. } => {
            b"omega.psi-rule.wrapping-integer-shift-right-constants.v1"
        }
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
