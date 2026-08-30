//! Optimizer module role: executable entrance. Wrapping integer-arithmetic source grammar coordination.

pub(in crate::validation::straight_line_parameter) mod wrapping_add;
pub(in crate::validation::straight_line_parameter) mod wrapping_subtract;

use omega_abstract_operations::{AbstractFunction, AbstractOperation};
use psi_core::ScalarType;

use super::super::super::model::IntegerArithmeticParametersSource;
use crate::validation::model::{
    StraightLineWrappingIntegerAddParametersTranslationError,
    StraightLineWrappingIntegerSubtractParametersTranslationError,
};

pub(in crate::validation::straight_line_parameter) fn reconstruct_wrapping_add(
    function: &AbstractFunction,
) -> Result<
    IntegerArithmeticParametersSource,
    StraightLineWrappingIntegerAddParametersTranslationError,
> {
    let Some(AbstractOperation::WrappingIntegerAdd { scalar_type, .. }) =
        function.operations.first()
    else {
        return Err(
            StraightLineWrappingIntegerAddParametersTranslationError::SourceOperationRoster,
        );
    };
    let envelope =
        super::super::envelope::reconstruct(function, ScalarType::Integer(*scalar_type))?;
    wrapping_add::reconstruct(function, &envelope)
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_wrapping_subtract(
    function: &AbstractFunction,
) -> Result<
    IntegerArithmeticParametersSource,
    StraightLineWrappingIntegerSubtractParametersTranslationError,
> {
    let Some(AbstractOperation::WrappingIntegerSubtract { scalar_type, .. }) =
        function.operations.first()
    else {
        return Err(
            StraightLineWrappingIntegerSubtractParametersTranslationError::SourceOperationRoster,
        );
    };
    let envelope =
        super::super::envelope::reconstruct(function, ScalarType::Integer(*scalar_type))?;
    wrapping_subtract::reconstruct(function, &envelope)
}
