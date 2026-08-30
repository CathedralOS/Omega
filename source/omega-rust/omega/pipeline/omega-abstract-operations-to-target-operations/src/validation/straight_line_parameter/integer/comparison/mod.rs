//! Optimizer module role: executable entrance. Integer-comparison source, ABI, provenance, and typed-target replay entrance.

pub(crate) mod equal;
pub(crate) mod less_or_equal;
pub(crate) mod less_than;
mod replay;

use super::super::{model::ReconstructedIntegerBinaryBooleanParameters, source};
use crate::validation::model::{
    StraightLineIntegerEqualParametersTranslationError,
    StraightLineIntegerLessOrEqualParametersTranslationError,
    StraightLineIntegerLessThanParametersTranslationError,
};
use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

pub(super) fn reconstruct_equal(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    ReconstructedIntegerBinaryBooleanParameters,
    StraightLineIntegerEqualParametersTranslationError,
> {
    replay::reconstruct(
        function,
        expected_target,
        target,
        source::integer::comparison::reconstruct_equal,
        StraightLineIntegerEqualParametersTranslationError::TargetProvenance,
    )
}

pub(super) fn reconstruct_less_than(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    ReconstructedIntegerBinaryBooleanParameters,
    StraightLineIntegerLessThanParametersTranslationError,
> {
    replay::reconstruct(
        function,
        expected_target,
        target,
        source::integer::comparison::reconstruct_less_than,
        StraightLineIntegerLessThanParametersTranslationError::TargetProvenance,
    )
}

pub(super) fn reconstruct_less_or_equal(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    ReconstructedIntegerBinaryBooleanParameters,
    StraightLineIntegerLessOrEqualParametersTranslationError,
> {
    replay::reconstruct(
        function,
        expected_target,
        target,
        source::integer::comparison::reconstruct_less_or_equal,
        StraightLineIntegerLessOrEqualParametersTranslationError::TargetProvenance,
    )
}
