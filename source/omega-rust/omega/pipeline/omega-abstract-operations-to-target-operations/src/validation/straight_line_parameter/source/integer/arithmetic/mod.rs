//! Optimizer module role: executable entrance. Exact arithmetic-family source replay routes.

pub(in crate::validation::straight_line_parameter) mod exact_add;
pub(in crate::validation::straight_line_parameter) mod reconstruction;
pub(in crate::validation::straight_line_parameter) mod saturating_add;
pub(in crate::validation::straight_line_parameter) mod wrapping_add;
pub(in crate::validation::straight_line_parameter) mod wrapping_multiply;
pub(in crate::validation::straight_line_parameter) mod wrapping_subtract;

use omega_abstract_operations::AbstractFunction;

pub(in crate::validation::straight_line_parameter) fn reconstruct_exact_add(
    function: &AbstractFunction,
) -> Result<
    super::super::super::model::ExactIntegerAddParametersSource,
    crate::validation::model::StraightLineExactIntegerAddParametersTranslationError,
> {
    reconstruction::reconstruct_exact_add(function)
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_saturating_add(
    function: &AbstractFunction,
) -> Result<
    super::super::super::model::IntegerArithmeticParametersSource,
    crate::validation::model::StraightLineSaturatingIntegerAddParametersTranslationError,
> {
    reconstruction::reconstruct_saturating_add(function)
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_wrapping_add(
    function: &AbstractFunction,
) -> Result<
    super::super::super::model::IntegerArithmeticParametersSource,
    crate::validation::model::StraightLineWrappingIntegerAddParametersTranslationError,
> {
    reconstruction::reconstruct_wrapping_add(function)
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_wrapping_subtract(
    function: &AbstractFunction,
) -> Result<
    super::super::super::model::IntegerArithmeticParametersSource,
    crate::validation::model::StraightLineWrappingIntegerSubtractParametersTranslationError,
> {
    reconstruction::reconstruct_wrapping_subtract(function)
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_wrapping_multiply(
    function: &AbstractFunction,
) -> Result<
    super::super::super::model::IntegerArithmeticParametersSource,
    crate::validation::model::StraightLineWrappingIntegerMultiplyParametersTranslationError,
> {
    reconstruction::reconstruct_wrapping_multiply(function)
}
