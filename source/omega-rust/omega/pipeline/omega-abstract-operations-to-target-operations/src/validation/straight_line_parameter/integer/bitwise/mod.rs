//! Integer-result binary bitwise source, ABI, provenance, and target replay.

pub(crate) mod bitwise_and;
pub(crate) mod bitwise_or;
pub(crate) mod bitwise_xor;
mod replay;

use super::super::model::ReconstructedIntegerBitwiseParameters;
use crate::validation::model::{
    StraightLineIntegerBitwiseAndParametersTranslationError,
    StraightLineIntegerBitwiseOrParametersTranslationError,
    StraightLineIntegerBitwiseXorParametersTranslationError,
};
use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

pub(super) fn reconstruct_bitwise_and(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    ReconstructedIntegerBitwiseParameters,
    StraightLineIntegerBitwiseAndParametersTranslationError,
> {
    replay::reconstruct(
        function,
        expected_target,
        target,
        super::super::source::integer::bitwise::reconstruct_bitwise_and,
        StraightLineIntegerBitwiseAndParametersTranslationError::TargetProvenance,
    )
}

pub(super) fn reconstruct_bitwise_or(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    ReconstructedIntegerBitwiseParameters,
    StraightLineIntegerBitwiseOrParametersTranslationError,
> {
    replay::reconstruct(
        function,
        expected_target,
        target,
        super::super::source::integer::bitwise::reconstruct_bitwise_or,
        StraightLineIntegerBitwiseOrParametersTranslationError::TargetProvenance,
    )
}

pub(super) fn reconstruct_bitwise_xor(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    ReconstructedIntegerBitwiseParameters,
    StraightLineIntegerBitwiseXorParametersTranslationError,
> {
    replay::reconstruct(
        function,
        expected_target,
        target,
        super::super::source::integer::bitwise::reconstruct_bitwise_xor,
        StraightLineIntegerBitwiseXorParametersTranslationError::TargetProvenance,
    )
}
