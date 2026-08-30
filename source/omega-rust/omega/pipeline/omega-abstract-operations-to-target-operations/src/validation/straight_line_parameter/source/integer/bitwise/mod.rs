//! Integer-result binary bitwise grammar coordination.

pub(in crate::validation::straight_line_parameter) mod bitwise_and;
pub(in crate::validation::straight_line_parameter) mod bitwise_or;

use omega_abstract_operations::{AbstractFunction, AbstractOperation};
use psi_core::ScalarType;

use super::super::super::model::IntegerBitwiseParametersSource;
use crate::validation::model::StraightLineIntegerBitwiseAndParametersTranslationError;
use crate::validation::model::StraightLineIntegerBitwiseOrParametersTranslationError;

pub(in crate::validation::straight_line_parameter) fn reconstruct_bitwise_and(
    function: &AbstractFunction,
) -> Result<IntegerBitwiseParametersSource, StraightLineIntegerBitwiseAndParametersTranslationError>
{
    let Some(AbstractOperation::IntegerBitwiseAnd { scalar_type, .. }) =
        function.operations.first()
    else {
        return Err(StraightLineIntegerBitwiseAndParametersTranslationError::SourceOperationRoster);
    };
    let envelope =
        super::super::envelope::reconstruct(function, ScalarType::Integer(*scalar_type))?;
    bitwise_and::reconstruct(function, &envelope)
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_bitwise_or(
    function: &AbstractFunction,
) -> Result<IntegerBitwiseParametersSource, StraightLineIntegerBitwiseOrParametersTranslationError>
{
    let Some(AbstractOperation::IntegerBitwiseOr { scalar_type, .. }) = function.operations.first()
    else {
        return Err(StraightLineIntegerBitwiseOrParametersTranslationError::SourceOperationRoster);
    };
    let envelope =
        super::super::envelope::reconstruct(function, ScalarType::Integer(*scalar_type))?;
    bitwise_or::reconstruct(function, &envelope)
}
