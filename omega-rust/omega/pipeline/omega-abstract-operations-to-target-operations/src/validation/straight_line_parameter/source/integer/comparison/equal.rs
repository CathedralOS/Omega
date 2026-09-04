//! Exact `[IntegerEqual(parameter, parameter), Return]` source replay.

use omega_abstract_operations::{AbstractFunction, AbstractOperation};
use psi_core::ScalarType;

use super::super::super::super::model::{
    IntegerBinaryBooleanParametersSource, ParameterResultKind, ReconstructedEnvelope,
};
use crate::validation::model::StraightLineIntegerEqualParametersTranslationError;

pub(in crate::validation::straight_line_parameter) fn is_candidate(
    function: &AbstractFunction,
) -> bool {
    super::super::super::has_candidate_envelope(function, ParameterResultKind::Boolean)
        && matches!(
            function.operations.as_slice(),
            [
                AbstractOperation::IntegerEqual { .. },
                AbstractOperation::Return {
                    cleanup_actions,
                    ..
                }
            ] if cleanup_actions.is_empty()
        )
}

pub(super) fn reconstruct(
    function: &AbstractFunction,
    envelope: &ReconstructedEnvelope<'_>,
) -> Result<IntegerBinaryBooleanParametersSource, StraightLineIntegerEqualParametersTranslationError>
{
    let [
        AbstractOperation::IntegerEqual {
            psi_operation,
            result: equal_result,
            left,
            right,
        },
        AbstractOperation::Return {
            psi_edge,
            result,
            value,
            scalar_type,
            cleanup_actions,
        },
    ] = function.operations.as_slice()
    else {
        return Err(StraightLineIntegerEqualParametersTranslationError::SourceOperationRoster);
    };
    if !cleanup_actions.is_empty() {
        return Err(StraightLineIntegerEqualParametersTranslationError::SourceCleanup);
    }
    if *result != envelope.function_result
        || *scalar_type != ScalarType::Boolean
        || *value != *equal_result
    {
        return Err(StraightLineIntegerEqualParametersTranslationError::SourceReturnLink);
    }
    if envelope
        .parameters
        .iter()
        .any(|parameter| parameter.value == *equal_result)
    {
        return Err(StraightLineIntegerEqualParametersTranslationError::SourceEqualResultRoster);
    }
    let (left_parameter_index, left_type) = super::super::parameter(envelope, *left)
        .ok_or(StraightLineIntegerEqualParametersTranslationError::SourceLeftOperandLink)?;
    let (right_parameter_index, right_type) = super::super::parameter(envelope, *right)
        .ok_or(StraightLineIntegerEqualParametersTranslationError::SourceRightOperandLink)?;
    if left_type != right_type {
        return Err(StraightLineIntegerEqualParametersTranslationError::SourceOperandTypeMismatch);
    }
    Ok(IntegerBinaryBooleanParametersSource {
        operation: *psi_operation,
        return_edge: *psi_edge,
        source_value: *equal_result,
        scalar_type: left_type,
        left_value: *left,
        right_value: *right,
        left_parameter_index,
        right_parameter_index,
    })
}
