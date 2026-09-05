//! Source reconstruction joins for proof-bearing quotient and remainder policies.

use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::ScalarType;

use super::super::super::super::super::model::{
    ExactIntegerDivideParametersSource, ExactIntegerRemainderParametersSource,
    SaturatingIntegerDivideParametersSource, SaturatingIntegerRemainderParametersSource,
    WrappingIntegerDivideParametersSource, WrappingIntegerRemainderParametersSource,
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
) -> Result<
    ExactIntegerDivideParametersSource,
    StraightLineExactIntegerDivideParametersTranslationError,
> {
    let Some(AbstractOperation::ExactIntegerDivide { scalar_type, .. }) =
        function.operations.first()
    else {
        return Err(
            StraightLineExactIntegerDivideParametersTranslationError::SourceOperationRoster,
        );
    };
    let envelope = super::super::super::super::envelope::reconstruct(
        function,
        ScalarType::Integer(*scalar_type),
    )?;
    super::super::exact_divide::reconstruct(function, &envelope)
}
pub(in crate::validation::straight_line_parameter) fn reconstruct_exact_remainder(
    function: &AbstractFunction,
) -> Result<
    ExactIntegerRemainderParametersSource,
    StraightLineExactIntegerRemainderParametersTranslationError,
> {
    let Some(AbstractOperation::ExactIntegerRemainder { scalar_type, .. }) =
        function.operations.first()
    else {
        return Err(
            StraightLineExactIntegerRemainderParametersTranslationError::SourceOperationRoster,
        );
    };
    let envelope = super::super::super::super::envelope::reconstruct(
        function,
        ScalarType::Integer(*scalar_type),
    )?;
    super::super::exact_remainder::reconstruct(function, &envelope)
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_wrapping_divide(
    function: &AbstractFunction,
) -> Result<
    WrappingIntegerDivideParametersSource,
    StraightLineWrappingIntegerDivideParametersTranslationError,
> {
    let Some(AbstractOperation::WrappingIntegerDivide { scalar_type, .. }) =
        function.operations.first()
    else {
        return Err(
            StraightLineWrappingIntegerDivideParametersTranslationError::SourceOperationRoster,
        );
    };
    let envelope = super::super::super::super::envelope::reconstruct(
        function,
        ScalarType::Integer(*scalar_type),
    )?;
    super::super::wrapping_divide::reconstruct(function, &envelope)
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_wrapping_remainder(
    function: &AbstractFunction,
) -> Result<
    WrappingIntegerRemainderParametersSource,
    StraightLineWrappingIntegerRemainderParametersTranslationError,
> {
    let Some(AbstractOperation::WrappingIntegerRemainder { scalar_type, .. }) =
        function.operations.first()
    else {
        return Err(
            StraightLineWrappingIntegerRemainderParametersTranslationError::SourceOperationRoster,
        );
    };
    let envelope = super::super::super::super::envelope::reconstruct(
        function,
        ScalarType::Integer(*scalar_type),
    )?;
    super::super::wrapping_remainder::reconstruct(function, &envelope)
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_saturating_divide(
    function: &AbstractFunction,
) -> Result<
    SaturatingIntegerDivideParametersSource,
    StraightLineSaturatingIntegerDivideParametersTranslationError,
> {
    let Some(AbstractOperation::SaturatingIntegerDivide { scalar_type, .. }) =
        function.operations.first()
    else {
        return Err(
            StraightLineSaturatingIntegerDivideParametersTranslationError::SourceOperationRoster,
        );
    };
    let envelope = super::super::super::super::envelope::reconstruct(
        function,
        ScalarType::Integer(*scalar_type),
    )?;
    super::super::saturating_divide::reconstruct(function, &envelope)
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_saturating_remainder(
    function: &AbstractFunction,
) -> Result<
    SaturatingIntegerRemainderParametersSource,
    StraightLineSaturatingIntegerRemainderParametersTranslationError,
> {
    let Some(AbstractOperation::SaturatingIntegerRemainder { scalar_type, .. }) =
        function.operations.first()
    else {
        return Err(
            StraightLineSaturatingIntegerRemainderParametersTranslationError::SourceOperationRoster,
        );
    };
    let envelope = super::super::super::super::envelope::reconstruct(
        function,
        ScalarType::Integer(*scalar_type),
    )?;
    super::super::saturating_remainder::reconstruct(function, &envelope)
}
