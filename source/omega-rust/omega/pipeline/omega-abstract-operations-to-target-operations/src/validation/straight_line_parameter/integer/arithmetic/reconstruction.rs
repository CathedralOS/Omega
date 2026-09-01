//! Shared arithmetic source, ABI, provenance, and target reconstruction.

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;

use super::super::super::model::{
    ReconstructedExactIntegerAddParameters, ReconstructedIntegerArithmeticParameters,
};
use crate::validation::model::{
    StraightLineExactIntegerAddParametersTranslationError,
    StraightLineSaturatingIntegerAddParametersTranslationError,
    StraightLineSaturatingIntegerMultiplyParametersTranslationError,
    StraightLineSaturatingIntegerSubtractParametersTranslationError,
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

macro_rules! reconstruct_arithmetic {
    ($name:ident, $source:ident, $error:ty) => {
        pub(super) fn $name(
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
