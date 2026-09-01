//! Shift source-envelope selection and typed replay join.

use omega_abstract_operations::{AbstractFunction, AbstractOperation};
use psi_core::ScalarType;

use super::super::super::super::model::IntegerShiftParametersSource;
use crate::validation::model::StraightLineWrappingIntegerShiftLeftParametersTranslationError;

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
