//! Optimizer module role: executable entrance. Exact and wrapping integer-arithmetic source, ABI, provenance, and target replay.

pub(crate) mod exact_add;
mod replay;
pub(crate) mod wrapping_add;
pub(crate) mod wrapping_multiply;
pub(crate) mod wrapping_subtract;

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

use super::super::model::{
    ReconstructedExactIntegerAddParameters, ReconstructedIntegerArithmeticParameters,
};
use crate::validation::model::{
    StraightLineExactIntegerAddParametersTranslationError,
    StraightLineWrappingIntegerAddParametersTranslationError,
    StraightLineWrappingIntegerMultiplyParametersTranslationError,
    StraightLineWrappingIntegerSubtractParametersTranslationError,
};

pub(super) fn reconstruct_exact_add(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    ReconstructedExactIntegerAddParameters,
    StraightLineExactIntegerAddParametersTranslationError,
> {
    let source = super::super::source::integer::arithmetic::reconstruct_exact_add(function)?;
    let arithmetic = replay::reconstruct_from_source(
        function,
        expected_target,
        target,
        source.arithmetic,
        StraightLineExactIntegerAddParametersTranslationError::TargetProvenance,
    )?;
    Ok(ReconstructedExactIntegerAddParameters {
        arithmetic,
        obligation: source.obligation,
    })
}

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

pub(super) fn reconstruct_wrapping_multiply(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    ReconstructedIntegerArithmeticParameters,
    StraightLineWrappingIntegerMultiplyParametersTranslationError,
> {
    replay::reconstruct(
        function,
        expected_target,
        target,
        super::super::source::integer::arithmetic::reconstruct_wrapping_multiply,
        StraightLineWrappingIntegerMultiplyParametersTranslationError::TargetProvenance,
    )
}
