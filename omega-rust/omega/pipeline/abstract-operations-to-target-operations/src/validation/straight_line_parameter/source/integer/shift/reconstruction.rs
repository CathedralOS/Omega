//! Shift source-envelope selection and typed replay join.

use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::ScalarType;

use super::super::super::super::model::{
    ExactIntegerShiftLeftParametersSource, ExactIntegerShiftRightParametersSource,
    IntegerShiftParametersSource,
};
use crate::validation::model::StraightLineExactIntegerShiftLeftParametersTranslationError;
use crate::validation::model::StraightLineExactIntegerShiftRightParametersTranslationError;
use crate::validation::model::StraightLineWrappingIntegerShiftLeftParametersTranslationError;
use crate::validation::model::StraightLineWrappingIntegerShiftRightParametersTranslationError;

pub(in crate::validation::straight_line_parameter) fn reconstruct_exact_left(
    function: &AbstractFunction,
) -> Result<
    ExactIntegerShiftLeftParametersSource,
    StraightLineExactIntegerShiftLeftParametersTranslationError,
> {
    let Some(AbstractOperation::ExactIntegerShiftLeft { value_type, .. }) =
        function.operations.first()
    else {
        return Err(
            StraightLineExactIntegerShiftLeftParametersTranslationError::SourceOperationRoster,
        );
    };
    let envelope =
        super::super::super::envelope::reconstruct(function, ScalarType::Integer(*value_type))?;
    super::exact_left::reconstruct(function, &envelope)
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_exact_right(
    function: &AbstractFunction,
) -> Result<
    ExactIntegerShiftRightParametersSource,
    StraightLineExactIntegerShiftRightParametersTranslationError,
> {
    let Some(AbstractOperation::ExactIntegerShiftRight { value_type, .. }) =
        function.operations.first()
    else {
        return Err(
            StraightLineExactIntegerShiftRightParametersTranslationError::SourceOperationRoster,
        );
    };
    let envelope =
        super::super::super::envelope::reconstruct(function, ScalarType::Integer(*value_type))?;
    super::exact_right::reconstruct(function, &envelope)
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_wrapping_left(
    function: &AbstractFunction,
) -> Result<
    IntegerShiftParametersSource,
    StraightLineWrappingIntegerShiftLeftParametersTranslationError,
> {
    let Some(AbstractOperation::WrappingIntegerShiftLeft { value_type, .. }) =
        function.operations.first()
    else {
        return Err(
            StraightLineWrappingIntegerShiftLeftParametersTranslationError::SourceOperationRoster,
        );
    };
    let envelope =
        super::super::super::envelope::reconstruct(function, ScalarType::Integer(*value_type))?;
    super::wrapping_left::reconstruct(function, &envelope)
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_wrapping_right(
    function: &AbstractFunction,
) -> Result<
    IntegerShiftParametersSource,
    StraightLineWrappingIntegerShiftRightParametersTranslationError,
> {
    let Some(AbstractOperation::WrappingIntegerShiftRight { value_type, .. }) =
        function.operations.first()
    else {
        return Err(
            StraightLineWrappingIntegerShiftRightParametersTranslationError::SourceOperationRoster,
        );
    };
    let envelope =
        super::super::super::envelope::reconstruct(function, ScalarType::Integer(*value_type))?;
    super::wrapping_right::reconstruct(function, &envelope)
}
