//! Integer-result unary grammar coordination.

pub(in crate::validation::straight_line_parameter) mod bitwise_not;
pub(in crate::validation::straight_line_parameter) mod widen;

use omega_abstract_operations::{AbstractFunction, AbstractOperation};
use psi_core::ScalarType;

use super::super::super::model::{IntegerUnaryParameterSource, IntegerWidenParameterSource};
use crate::validation::model::{
    StraightLineIntegerBitwiseNotParameterTranslationError,
    StraightLineIntegerWidenParameterTranslationError,
};

pub(in crate::validation::straight_line_parameter) fn reconstruct_bitwise_not(
    function: &AbstractFunction,
) -> Result<IntegerUnaryParameterSource, StraightLineIntegerBitwiseNotParameterTranslationError> {
    let Some(AbstractOperation::IntegerBitwiseNot { scalar_type, .. }) =
        function.operations.first()
    else {
        return Err(StraightLineIntegerBitwiseNotParameterTranslationError::SourceOperationRoster);
    };
    let envelope =
        super::super::envelope::reconstruct(function, ScalarType::Integer(*scalar_type))?;
    bitwise_not::reconstruct(function, &envelope)
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_widen(
    function: &AbstractFunction,
) -> Result<IntegerWidenParameterSource, StraightLineIntegerWidenParameterTranslationError> {
    let Some(AbstractOperation::IntegerWiden { target_type, .. }) = function.operations.first()
    else {
        return Err(StraightLineIntegerWidenParameterTranslationError::SourceOperationRoster);
    };
    let envelope =
        super::super::envelope::reconstruct(function, ScalarType::Integer(*target_type))?;
    widen::reconstruct(function, &envelope)
}
