//! Whole-plan identity, evidence-roster, and function-roster replay.


use abstract_operations::{AbstractOperation, AbstractOperationPlan};

use super::AbstractToTargetTranslationValidationError;

/// Raw IEEE fused multiply-add has no target realization; reject it even when
/// the target rows omit it.
pub(crate) fn reject_raw_ieee_float_fma(
    source: &AbstractOperationPlan,
) -> Result<(), AbstractToTargetTranslationValidationError> {
    match source
        .operations()
        .filter_map(|operation| match operation {
            AbstractOperation::NearestIeeeFloatFusedMultiplyAdd { psi_operation, .. } => {
                Some(*psi_operation)
            }
            _ => None,
        })
        .min()
    {
        Some(operation) => Err(
            AbstractToTargetTranslationValidationError::UnsupportedIeeeFloatFmaRealization(
                operation,
            ),
        ),
        None => Ok(()),
    }
}

