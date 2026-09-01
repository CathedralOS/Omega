//! Shared arithmetic source, ABI, provenance, and target reconstruction.

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

use super::super::super::model::{
    ReconstructedExactIntegerAddParameters, ReconstructedExactIntegerDivideParameters,
    ReconstructedExactIntegerMultiplyParameters, ReconstructedExactIntegerRemainderParameters,
    ReconstructedExactIntegerSubtractParameters, ReconstructedIntegerArithmeticParameters,
    ReconstructedWrappingIntegerDivideParameters,
};
use crate::validation::model::{
    StraightLineExactIntegerAddParametersTranslationError,
    StraightLineExactIntegerDivideParametersTranslationError,
    StraightLineExactIntegerMultiplyParametersTranslationError,
    StraightLineExactIntegerRemainderParametersTranslationError,
    StraightLineExactIntegerSubtractParametersTranslationError,
    StraightLineSaturatingIntegerAddParametersTranslationError,
    StraightLineSaturatingIntegerMultiplyParametersTranslationError,
    StraightLineSaturatingIntegerSubtractParametersTranslationError,
    StraightLineWrappingIntegerAddParametersTranslationError,
    StraightLineWrappingIntegerDivideParametersTranslationError,
    StraightLineWrappingIntegerMultiplyParametersTranslationError,
    StraightLineWrappingIntegerSubtractParametersTranslationError,
};

pub(in crate::validation::straight_line_parameter) fn reconstruct_exact_add(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    ReconstructedExactIntegerAddParameters,
    StraightLineExactIntegerAddParametersTranslationError,
> {
    let source = super::super::super::source::integer::arithmetic::reconstruct_exact_add(function)?;
    let arithmetic = super::replay::reconstruct_from_source(
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

pub(in crate::validation::straight_line_parameter) fn reconstruct_exact_subtract(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    ReconstructedExactIntegerSubtractParameters,
    StraightLineExactIntegerSubtractParametersTranslationError,
> {
    let source =
        super::super::super::source::integer::arithmetic::reconstruct_exact_subtract(function)?;
    let arithmetic = super::replay::reconstruct_from_source(
        function,
        expected_target,
        target,
        source.arithmetic,
        StraightLineExactIntegerSubtractParametersTranslationError::TargetProvenance,
    )?;
    Ok(ReconstructedExactIntegerSubtractParameters {
        arithmetic,
        obligation: source.obligation,
    })
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_exact_multiply(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    ReconstructedExactIntegerMultiplyParameters,
    StraightLineExactIntegerMultiplyParametersTranslationError,
> {
    let source =
        super::super::super::source::integer::arithmetic::reconstruct_exact_multiply(function)?;
    let arithmetic = super::replay::reconstruct_from_source(
        function,
        expected_target,
        target,
        source.arithmetic,
        StraightLineExactIntegerMultiplyParametersTranslationError::TargetProvenance,
    )?;
    Ok(ReconstructedExactIntegerMultiplyParameters {
        arithmetic,
        obligation: source.obligation,
    })
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_exact_divide(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    ReconstructedExactIntegerDivideParameters,
    StraightLineExactIntegerDivideParametersTranslationError,
> {
    let source =
        super::super::super::source::integer::arithmetic::reconstruct_exact_divide(function)?;
    let arithmetic = super::replay::reconstruct_from_source(
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
        super::super::super::source::integer::arithmetic::reconstruct_exact_remainder(function)?;
    let arithmetic = super::replay::reconstruct_from_source(
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
        super::super::super::source::integer::arithmetic::reconstruct_wrapping_divide(function)?;
    let arithmetic = super::replay::reconstruct_from_source(
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

macro_rules! reconstruct_arithmetic {
    ($name:ident, $source:ident, $error:ty) => {
        pub(in crate::validation::straight_line_parameter) fn $name(
            function: &AbstractFunction,
            expected_target: NativeTarget,
            target: &TargetFunction,
        ) -> Result<ReconstructedIntegerArithmeticParameters, $error> {
            super::replay::reconstruct(
                function,
                expected_target,
                target,
                super::super::super::source::integer::arithmetic::$source,
                <$error>::TargetProvenance,
            )
        }
    };
}

reconstruct_arithmetic!(
    reconstruct_saturating_add,
    reconstruct_saturating_add,
    StraightLineSaturatingIntegerAddParametersTranslationError
);
reconstruct_arithmetic!(
    reconstruct_saturating_subtract,
    reconstruct_saturating_subtract,
    StraightLineSaturatingIntegerSubtractParametersTranslationError
);
reconstruct_arithmetic!(
    reconstruct_saturating_multiply,
    reconstruct_saturating_multiply,
    StraightLineSaturatingIntegerMultiplyParametersTranslationError
);
reconstruct_arithmetic!(
    reconstruct_wrapping_add,
    reconstruct_wrapping_add,
    StraightLineWrappingIntegerAddParametersTranslationError
);
reconstruct_arithmetic!(
    reconstruct_wrapping_subtract,
    reconstruct_wrapping_subtract,
    StraightLineWrappingIntegerSubtractParametersTranslationError
);
reconstruct_arithmetic!(
    reconstruct_wrapping_multiply,
    reconstruct_wrapping_multiply,
    StraightLineWrappingIntegerMultiplyParametersTranslationError
);
