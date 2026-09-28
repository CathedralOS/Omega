//! Independent operation-to-rule identity replay for literal Boolean-result evaluation.

use abstract_operations::AbstractOperation as O;
use optimization_core::OptimizationRuleIdentity;

use crate::OptimizationUnitValidationError;

pub(super) fn validate(
    operation: &O,
    actual: OptimizationRuleIdentity,
) -> Result<(), OptimizationUnitValidationError> {
    let canonical_name: &[u8] = match operation {
        O::BooleanNot { .. } => b"omega.psi-rule.boolean-not-constants.v1",
        O::BooleanEqual { .. } => b"omega.psi-rule.boolean-equal-constants.v1",
        O::IntegerEqual { .. } => b"omega.psi-rule.integer-equal-constants.v1",
        O::IntegerLessThan { .. } => b"omega.psi-rule.integer-less-than-constants.v1",
        O::IntegerLessOrEqual { .. } => b"omega.psi-rule.integer-less-or-equal-constants.v1",
        _ => return Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    };
    let expected = OptimizationRuleIdentity::from_canonical_bytes(canonical_name);
    if actual != expected {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    Ok(())
}
