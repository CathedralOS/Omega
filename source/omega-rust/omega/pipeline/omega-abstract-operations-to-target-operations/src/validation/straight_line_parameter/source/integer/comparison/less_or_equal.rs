//! Exact `[IntegerLessOrEqual(parameter, parameter), Return]` source replay.

use omega_abstract_operations::{AbstractFunction, AbstractOperation};
use psi_core::ScalarType;

use super::super::super::super::model::{
    IntegerBinaryBooleanParametersSource, ParameterResultKind, ReconstructedEnvelope,
};
use crate::validation::model::StraightLineIntegerLessOrEqualParametersTranslationError;

pub(in crate::validation::straight_line_parameter) fn is_candidate(
    function: &AbstractFunction,
) -> bool {
    super::super::super::has_candidate_envelope(function, ParameterResultKind::Boolean)
        && matches!(
            function.operations.as_slice(),
            [
                AbstractOperation::IntegerLessOrEqual { .. },
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
    StraightLineIntegerLessOrEqualParametersTranslationError,
> {
    let [
        AbstractOperation::IntegerLessOrEqual {
            psi_operation,
            result: less_or_equal_result,
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
        return Err(
            StraightLineIntegerLessOrEqualParametersTranslationError::SourceOperationRoster,
        );
    };
    if !cleanup_actions.is_empty() {
        return Err(StraightLineIntegerLessOrEqualParametersTranslationError::SourceCleanup);
    }
    if *result != envelope.function_result
        || *scalar_type != ScalarType::Boolean
        || *value != *less_or_equal_result
    {
        return Err(StraightLineIntegerLessOrEqualParametersTranslationError::SourceReturnLink);
    }
    if envelope
        .parameters
        .iter()
        .any(|parameter| parameter.value == *less_or_equal_result)
    {
        return Err(
            StraightLineIntegerLessOrEqualParametersTranslationError::SourceLessOrEqualResultRoster,
        );
    }
    let (left_parameter_index, left_type) = super::super::parameter(envelope, *left)
        .ok_or(StraightLineIntegerLessOrEqualParametersTranslationError::SourceLeftOperandLink)?;
    let (right_parameter_index, right_type) = super::super::parameter(envelope, *right)
        .ok_or(StraightLineIntegerLessOrEqualParametersTranslationError::SourceRightOperandLink)?;
    if left_type != right_type {
        return Err(
            StraightLineIntegerLessOrEqualParametersTranslationError::SourceOperandTypeMismatch,
        );
    }
    Ok(IntegerBinaryBooleanParametersSource {
        operation: *psi_operation,
        return_edge: *psi_edge,
        source_value: *less_or_equal_result,
        scalar_type: left_type,
        left_value: *left,
        right_value: *right,
        left_parameter_index,
        right_parameter_index,
    })
}
