//! Source-only grammar map for direct and derived parameter use.

pub(super) mod boolean_equal;
pub(super) mod boolean_not;
pub(super) mod direct;
mod envelope;
pub(super) mod integer_equal;
pub(super) mod integer_less_than;

use omega_abstract_operations::AbstractFunction;
use psi_core::{IntegerType, ScalarType, ValueId};

use super::super::StraightLineBooleanEqualParametersTranslationError;
use super::super::StraightLineBooleanNotParameterTranslationError;
use super::super::StraightLineIntegerEqualParametersTranslationError;
use super::super::StraightLineIntegerLessThanParametersTranslationError;
use super::{
    StraightLineParameterReconstructionError,
    model::{
        BooleanEqualParametersSource, BooleanNotParameterSource,
        IntegerBinaryBooleanParametersSource, ParameterResultKind, ParameterReturnSource,
        ReconstructedEnvelope,
    },
};

pub(super) fn has_candidate_envelope(
    function: &AbstractFunction,
    result_kind: ParameterResultKind,
) -> bool {
    envelope::is_candidate(function, result_kind)
}

pub(super) fn reconstruct_direct(
    function: &AbstractFunction,
    expected_result: ScalarType,
) -> Result<ParameterReturnSource, StraightLineParameterReconstructionError> {
    let envelope = envelope::reconstruct(function, expected_result)?;
    direct::reconstruct(function, &envelope, expected_result)
}

pub(super) fn reconstruct_boolean_not(
    function: &AbstractFunction,
) -> Result<BooleanNotParameterSource, StraightLineBooleanNotParameterTranslationError> {
    let envelope = envelope::reconstruct(function, ScalarType::Boolean)?;
    boolean_not::reconstruct(function, &envelope)
}

pub(super) fn reconstruct_boolean_equal(
    function: &AbstractFunction,
) -> Result<BooleanEqualParametersSource, StraightLineBooleanEqualParametersTranslationError> {
    let envelope = envelope::reconstruct(function, ScalarType::Boolean)?;
    boolean_equal::reconstruct(function, &envelope)
}

pub(super) fn reconstruct_integer_equal(
    function: &AbstractFunction,
) -> Result<IntegerBinaryBooleanParametersSource, StraightLineIntegerEqualParametersTranslationError>
{
    let envelope = envelope::reconstruct(function, ScalarType::Boolean)?;
    integer_equal::reconstruct(function, &envelope)
}

pub(super) fn reconstruct_integer_less_than(
    function: &AbstractFunction,
) -> Result<
    IntegerBinaryBooleanParametersSource,
    StraightLineIntegerLessThanParametersTranslationError,
> {
    let envelope = envelope::reconstruct(function, ScalarType::Boolean)?;
    integer_less_than::reconstruct(function, &envelope)
}

pub(in crate::validation::straight_line_parameter::source) fn integer_parameter(
    envelope: &ReconstructedEnvelope<'_>,
    value: ValueId,
) -> Option<(usize, IntegerType)> {
    envelope
        .parameters
        .iter()
        .enumerate()
        .find_map(|(index, parameter)| match parameter.scalar_type {
            ScalarType::Integer(integer_type) if parameter.value == value => {
                Some((index, integer_type))
            }
            ScalarType::Integer(_) | ScalarType::Boolean => None,
        })
}
