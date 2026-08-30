//! Boolean-result integer comparison grammar coordination.

pub(in crate::validation::straight_line_parameter) mod equal;
pub(in crate::validation::straight_line_parameter) mod less_or_equal;
pub(in crate::validation::straight_line_parameter) mod less_than;

use omega_abstract_operations::AbstractFunction;
use psi_core::ScalarType;

use super::super::super::model::IntegerBinaryBooleanParametersSource;
use crate::validation::model::{
    StraightLineIntegerEqualParametersTranslationError,
    StraightLineIntegerLessOrEqualParametersTranslationError,
    StraightLineIntegerLessThanParametersTranslationError,
};

pub(in crate::validation::straight_line_parameter) fn reconstruct_equal(
    function: &AbstractFunction,
) -> Result<IntegerBinaryBooleanParametersSource, StraightLineIntegerEqualParametersTranslationError>
{
    let envelope = super::super::envelope::reconstruct(function, ScalarType::Boolean)?;
    equal::reconstruct(function, &envelope)
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_less_than(
    function: &AbstractFunction,
) -> Result<
    IntegerBinaryBooleanParametersSource,
    StraightLineIntegerLessThanParametersTranslationError,
> {
    let envelope = super::super::envelope::reconstruct(function, ScalarType::Boolean)?;
    less_than::reconstruct(function, &envelope)
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_less_or_equal(
    function: &AbstractFunction,
) -> Result<
    IntegerBinaryBooleanParametersSource,
    StraightLineIntegerLessOrEqualParametersTranslationError,
> {
    let envelope = super::super::envelope::reconstruct(function, ScalarType::Boolean)?;
    less_or_equal::reconstruct(function, &envelope)
}
