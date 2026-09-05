//! Optimizer module role: executable entrance. Join exact family enablement, typed selection, and whole-plan replay.
//! The adjacent `enabled_families` roster is the sole enable/disable seam; typed adapters remain under `dispatch`.

mod dispatch;
mod enabled_families;
mod model;
mod plan;
mod selection;

use abstract_operations::AbstractFunction;
use target::NativeTarget;
use target_operations::TargetFunction;

use super::{
    AbstractToTargetFunctionTranslationDisposition, AbstractToTargetTranslationValidationError,
};

pub(super) fn validate_function(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
    ieee_float_fma: &[crate::AdmittedIeeeFloatFmaSettlement<'_>],
) -> Result<
    AbstractToTargetFunctionTranslationDisposition,
    AbstractToTargetTranslationValidationError,
> {
    selection::validate_with_ieee_float_fma(
        source,
        expected_target,
        target,
        enabled_families::ENABLED_TRANSLATION_FAMILIES,
        ieee_float_fma,
    )
}

pub(super) use plan::validate as validate_plan;

#[cfg(test)]
mod tests;
