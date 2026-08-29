//! Typed integer-derived source grammar coordination.
//!
//! This entrance owns the common result-envelope and typed-parameter lookup,
//! then descends into one exact operation grammar.

pub(in crate::validation::straight_line_parameter) mod bitwise_not;
pub(in crate::validation::straight_line_parameter) mod equal;
pub(in crate::validation::straight_line_parameter) mod less_or_equal;
pub(in crate::validation::straight_line_parameter) mod less_than;

use omega_abstract_operations::{AbstractFunction, AbstractOperation};
use psi_core::{IntegerType, ScalarType, ValueId};

use super::super::model::{
    IntegerBinaryBooleanParametersSource, IntegerUnaryParameterSource, ReconstructedEnvelope,
};
use crate::validation::model::{
    StraightLineIntegerBitwiseNotParameterTranslationError,
    StraightLineIntegerEqualParametersTranslationError,
    StraightLineIntegerLessOrEqualParametersTranslationError,
    StraightLineIntegerLessThanParametersTranslationError,
};

pub(in crate::validation::straight_line_parameter) fn reconstruct_bitwise_not(
    function: &AbstractFunction,
) -> Result<IntegerUnaryParameterSource, StraightLineIntegerBitwiseNotParameterTranslationError> {
    let Some(AbstractOperation::IntegerBitwiseNot { scalar_type, .. }) =
        function.operations.first()
    else {
        return Err(StraightLineIntegerBitwiseNotParameterTranslationError::SourceOperationRoster);
    };
    let envelope = super::envelope::reconstruct(function, ScalarType::Integer(*scalar_type))?;
    bitwise_not::reconstruct(function, &envelope)
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_equal(
    function: &AbstractFunction,
) -> Result<IntegerBinaryBooleanParametersSource, StraightLineIntegerEqualParametersTranslationError>
{
    let envelope = super::envelope::reconstruct(function, ScalarType::Boolean)?;
    equal::reconstruct(function, &envelope)
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_less_than(
    function: &AbstractFunction,
) -> Result<
    IntegerBinaryBooleanParametersSource,
    StraightLineIntegerLessThanParametersTranslationError,
> {
    let envelope = super::envelope::reconstruct(function, ScalarType::Boolean)?;
    less_than::reconstruct(function, &envelope)
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_less_or_equal(
    function: &AbstractFunction,
) -> Result<
    IntegerBinaryBooleanParametersSource,
    StraightLineIntegerLessOrEqualParametersTranslationError,
> {
    let envelope = super::envelope::reconstruct(function, ScalarType::Boolean)?;
    less_or_equal::reconstruct(function, &envelope)
}

pub(in crate::validation::straight_line_parameter::source) fn parameter(
    envelope: &ReconstructedEnvelope<'_>,
    value: ValueId,
) -> Option<(usize, IntegerType)> {
    envelope
        .parameters
        .iter()
        .enumerate()
        .find_map(|(index, candidate)| match candidate.scalar_type {
            ScalarType::Integer(integer_type) if candidate.value == value => {
                Some((index, integer_type))
            }
            ScalarType::Integer(_) | ScalarType::Boolean => None,
        })
}
