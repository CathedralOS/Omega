//! Optimizer module role: executable entrance. Exact arithmetic-family target replay routes.

pub(crate) mod exact_add;
mod reconstruction;
mod replay;
pub(crate) mod saturating_add;
pub(crate) mod saturating_subtract;
pub(crate) mod wrapping_add;
pub(crate) mod wrapping_multiply;
pub(crate) mod wrapping_subtract;

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

pub(super) fn reconstruct_exact_add(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    super::super::model::ReconstructedExactIntegerAddParameters,
    crate::validation::model::StraightLineExactIntegerAddParametersTranslationError,
> {
    reconstruction::reconstruct_exact_add(function, expected_target, target)
}

pub(super) fn reconstruct_saturating_add(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    super::super::model::ReconstructedIntegerArithmeticParameters,
    crate::validation::model::StraightLineSaturatingIntegerAddParametersTranslationError,
> {
    reconstruction::reconstruct_saturating_add(function, expected_target, target)
}

pub(super) fn reconstruct_saturating_subtract(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    super::super::model::ReconstructedIntegerArithmeticParameters,
    crate::validation::model::StraightLineSaturatingIntegerSubtractParametersTranslationError,
> {
    reconstruction::reconstruct_saturating_subtract(function, expected_target, target)
}

pub(super) fn reconstruct_wrapping_add(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    super::super::model::ReconstructedIntegerArithmeticParameters,
    crate::validation::model::StraightLineWrappingIntegerAddParametersTranslationError,
> {
    reconstruction::reconstruct_wrapping_add(function, expected_target, target)
}

pub(super) fn reconstruct_wrapping_subtract(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    super::super::model::ReconstructedIntegerArithmeticParameters,
    crate::validation::model::StraightLineWrappingIntegerSubtractParametersTranslationError,
> {
    reconstruction::reconstruct_wrapping_subtract(function, expected_target, target)
}

pub(super) fn reconstruct_wrapping_multiply(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    super::super::model::ReconstructedIntegerArithmeticParameters,
    crate::validation::model::StraightLineWrappingIntegerMultiplyParametersTranslationError,
> {
    reconstruction::reconstruct_wrapping_multiply(function, expected_target, target)
}
