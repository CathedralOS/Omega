//! Target reconstruction joins for proof-bearing quotient and remainder policies.

use abstract_operations::AbstractFunction;
use target::NativeTarget;
use target_operations::TargetFunction;

use super::super::super::super::model::{
    ReconstructedExactIntegerDivideParameters, ReconstructedExactIntegerRemainderParameters,
    ReconstructedSaturatingIntegerDivideParameters,
    ReconstructedSaturatingIntegerRemainderParameters,
    ReconstructedWrappingIntegerDivideParameters, ReconstructedWrappingIntegerRemainderParameters,
};
use crate::validation::model::{
    StraightLineExactIntegerDivideParametersTranslationError,
    StraightLineExactIntegerRemainderParametersTranslationError,
    StraightLineSaturatingIntegerDivideParametersTranslationError,
    StraightLineSaturatingIntegerRemainderParametersTranslationError,
    StraightLineWrappingIntegerDivideParametersTranslationError,
    StraightLineWrappingIntegerRemainderParametersTranslationError,
};

pub(in crate::validation::straight_line_parameter) fn reconstruct_exact_divide(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    ReconstructedExactIntegerDivideParameters,
    StraightLineExactIntegerDivideParametersTranslationError,
> {
    let source = super::super::super::super::source::integer::arithmetic::reconstruct_exact_divide(
        function,
    )?;
    let arithmetic = super::super::replay::reconstruct_from_source(
        function,
        expected_target,
        target,
        source.arithmetic,
        StraightLineExactIntegerDivideParametersTranslationError::TargetProvenance,
    )?;
    Ok(ReconstructedExactIntegerDivideParameters {
        arithmetic,
        obligation: source.obligation,
    })
}
pub(in crate::validation::straight_line_parameter) fn reconstruct_exact_remainder(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    ReconstructedExactIntegerRemainderParameters,
    StraightLineExactIntegerRemainderParametersTranslationError,
> {
    let source =
        super::super::super::super::source::integer::arithmetic::reconstruct_exact_remainder(
            function,
        )?;
    let arithmetic = super::super::replay::reconstruct_from_source(
        function,
        expected_target,
        target,
        source.arithmetic,
        StraightLineExactIntegerRemainderParametersTranslationError::TargetProvenance,
    )?;
    Ok(ReconstructedExactIntegerRemainderParameters {
        arithmetic,
        obligation: source.obligation,
    })
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_wrapping_divide(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    ReconstructedWrappingIntegerDivideParameters,
    StraightLineWrappingIntegerDivideParametersTranslationError,
> {
    let source =
        super::super::super::super::source::integer::arithmetic::reconstruct_wrapping_divide(
            function,
        )?;
    let arithmetic = super::super::replay::reconstruct_from_source(
        function,
        expected_target,
        target,
        source.arithmetic,
        StraightLineWrappingIntegerDivideParametersTranslationError::TargetProvenance,
    )?;
    Ok(ReconstructedWrappingIntegerDivideParameters {
        arithmetic,
        obligation: source.obligation,
    })
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_wrapping_remainder(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    ReconstructedWrappingIntegerRemainderParameters,
    StraightLineWrappingIntegerRemainderParametersTranslationError,
> {
    let source =
        super::super::super::super::source::integer::arithmetic::reconstruct_wrapping_remainder(
            function,
        )?;
    let arithmetic = super::super::replay::reconstruct_from_source(
        function,
        expected_target,
        target,
        source.arithmetic,
        StraightLineWrappingIntegerRemainderParametersTranslationError::TargetProvenance,
    )?;
    Ok(ReconstructedWrappingIntegerRemainderParameters {
        arithmetic,
        obligation: source.obligation,
    })
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_saturating_divide(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    ReconstructedSaturatingIntegerDivideParameters,
    StraightLineSaturatingIntegerDivideParametersTranslationError,
> {
    let source =
        super::super::super::super::source::integer::arithmetic::reconstruct_saturating_divide(
            function,
        )?;
    let arithmetic = super::super::replay::reconstruct_from_source(
        function,
        expected_target,
        target,
        source.arithmetic,
        StraightLineSaturatingIntegerDivideParametersTranslationError::TargetProvenance,
    )?;
    Ok(ReconstructedSaturatingIntegerDivideParameters {
        arithmetic,
        obligation: source.obligation,
    })
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_saturating_remainder(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    ReconstructedSaturatingIntegerRemainderParameters,
    StraightLineSaturatingIntegerRemainderParametersTranslationError,
> {
    let source =
        super::super::super::super::source::integer::arithmetic::reconstruct_saturating_remainder(
            function,
        )?;
    let arithmetic = super::super::replay::reconstruct_from_source(
        function,
        expected_target,
        target,
        source.arithmetic,
        StraightLineSaturatingIntegerRemainderParametersTranslationError::TargetProvenance,
    )?;
    Ok(ReconstructedSaturatingIntegerRemainderParameters {
        arithmetic,
        obligation: source.obligation,
    })
}
