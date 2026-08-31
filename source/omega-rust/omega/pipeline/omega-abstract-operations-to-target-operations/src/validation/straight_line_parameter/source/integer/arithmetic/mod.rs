//! Optimizer module role: executable entrance. Exact and wrapping integer-arithmetic source grammar coordination.

pub(in crate::validation::straight_line_parameter) mod exact_add;
pub(in crate::validation::straight_line_parameter) mod wrapping_add;
pub(in crate::validation::straight_line_parameter) mod wrapping_multiply;
pub(in crate::validation::straight_line_parameter) mod wrapping_subtract;

use omega_abstract_operations::{AbstractFunction, AbstractOperation};
use psi_core::ScalarType;

use super::super::super::model::{
    ExactIntegerAddParametersSource, IntegerArithmeticParametersSource,
};
use crate::validation::model::{
    StraightLineExactIntegerAddParametersTranslationError,
    StraightLineWrappingIntegerAddParametersTranslationError,
    StraightLineWrappingIntegerMultiplyParametersTranslationError,
    StraightLineWrappingIntegerSubtractParametersTranslationError,
};

pub(in crate::validation::straight_line_parameter) fn reconstruct_exact_add(
    function: &AbstractFunction,
) -> Result<ExactIntegerAddParametersSource, StraightLineExactIntegerAddParametersTranslationError>
{
    let Some(AbstractOperation::ExactIntegerAdd { scalar_type, .. }) = function.operations.first()
    else {
        return Err(StraightLineExactIntegerAddParametersTranslationError::SourceOperationRoster);
    };
    let envelope =
        super::super::envelope::reconstruct(function, ScalarType::Integer(*scalar_type))?;
    exact_add::reconstruct(function, &envelope)
}

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

pub(in crate::validation::straight_line_parameter) fn reconstruct_wrapping_multiply(
    function: &AbstractFunction,
) -> Result<
    IntegerArithmeticParametersSource,
    StraightLineWrappingIntegerMultiplyParametersTranslationError,
> {
    let Some(AbstractOperation::WrappingIntegerMultiply { scalar_type, .. }) =
        function.operations.first()
    else {
        return Err(
            StraightLineWrappingIntegerMultiplyParametersTranslationError::SourceOperationRoster,
        );
    };
    let envelope =
        super::super::envelope::reconstruct(function, ScalarType::Integer(*scalar_type))?;
    wrapping_multiply::reconstruct(function, &envelope)
}
