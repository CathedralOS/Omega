//! Optimizer module role: executable entrance. Wrapping integer-arithmetic source, ABI, provenance, and target replay.

mod replay;
pub(crate) mod wrapping_add;
pub(crate) mod wrapping_subtract;

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

use super::super::model::ReconstructedIntegerArithmeticParameters;
use crate::validation::model::{
    StraightLineWrappingIntegerAddParametersTranslationError,
    StraightLineWrappingIntegerSubtractParametersTranslationError,
};

pub(super) fn reconstruct_wrapping_add(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    ReconstructedIntegerArithmeticParameters,
    StraightLineWrappingIntegerAddParametersTranslationError,
> {
    replay::reconstruct(
        function,
        expected_target,
        target,
        super::super::source::integer::arithmetic::reconstruct_wrapping_add,
        StraightLineWrappingIntegerAddParametersTranslationError::TargetProvenance,
    )
}

pub(super) fn reconstruct_wrapping_subtract(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    ReconstructedIntegerArithmeticParameters,
    StraightLineWrappingIntegerSubtractParametersTranslationError,
> {
    replay::reconstruct(
        function,
        expected_target,
        target,
        super::super::source::integer::arithmetic::reconstruct_wrapping_subtract,
        StraightLineWrappingIntegerSubtractParametersTranslationError::TargetProvenance,
    )
}
