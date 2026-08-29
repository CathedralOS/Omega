//! Exact `[IntegerLessThan(parameter, parameter), Return]` source replay.

use omega_abstract_operations::{AbstractFunction, AbstractOperation};
use psi_core::ScalarType;

use super::super::model::{
    IntegerBinaryBooleanParametersSource, ParameterResultKind, ReconstructedEnvelope,
};
use crate::validation::model::StraightLineIntegerLessThanParametersTranslationError;

pub(in crate::validation::straight_line_parameter) fn is_candidate(
    function: &AbstractFunction,
) -> bool {
    super::has_candidate_envelope(function, ParameterResultKind::Boolean)
        && matches!(
            function.operations.as_slice(),
            [
                AbstractOperation::IntegerLessThan { .. },
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
) -> Result<
    IntegerBinaryBooleanParametersSource,
    StraightLineIntegerLessThanParametersTranslationError,
> {
    let [
        AbstractOperation::IntegerLessThan {
            psi_operation,
            result: less_than_result,
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
        return Err(StraightLineIntegerLessThanParametersTranslationError::SourceOperationRoster);
    };
    if !cleanup_actions.is_empty() {
        return Err(StraightLineIntegerLessThanParametersTranslationError::SourceCleanup);
    }
    if *result != envelope.function_result
        || *scalar_type != ScalarType::Boolean
        || *value != *less_than_result
    {
        return Err(StraightLineIntegerLessThanParametersTranslationError::SourceReturnLink);
    }
    if envelope
        .parameters
        .iter()
        .any(|parameter| parameter.value == *less_than_result)
    {
        return Err(
            StraightLineIntegerLessThanParametersTranslationError::SourceLessThanResultRoster,
        );
    }
    let (left_parameter_index, left_type) = super::integer_parameter(envelope, *left)
        .ok_or(StraightLineIntegerLessThanParametersTranslationError::SourceLeftOperandLink)?;
    let (right_parameter_index, right_type) = super::integer_parameter(envelope, *right)
        .ok_or(StraightLineIntegerLessThanParametersTranslationError::SourceRightOperandLink)?;
    if left_type != right_type {
        return Err(
            StraightLineIntegerLessThanParametersTranslationError::SourceOperandTypeMismatch,
        );
    }
    Ok(IntegerBinaryBooleanParametersSource {
        operation: *psi_operation,
        return_edge: *psi_edge,
        source_value: *less_than_result,
        scalar_type: left_type,
        left_value: *left,
        right_value: *right,
        left_parameter_index,
        right_parameter_index,
    })
}
