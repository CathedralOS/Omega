//! Exact `[SaturatingIntegerSubtract(parameter, parameter), Return]` source replay.

use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::ScalarType;

use super::super::super::super::model::{
    IntegerArithmeticParametersSource, ParameterResultKind, ReconstructedEnvelope,
};
use crate::validation::model::StraightLineSaturatingIntegerSubtractParametersTranslationError;

pub(in crate::validation::straight_line_parameter) fn is_candidate(
    function: &AbstractFunction,
) -> bool {
    super::super::super::has_candidate_envelope(function, ParameterResultKind::Integer)
        && matches!(
            function.operations.as_slice(),
            [
                AbstractOperation::SaturatingIntegerSubtract { .. },
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
    IntegerArithmeticParametersSource,
    StraightLineSaturatingIntegerSubtractParametersTranslationError,
> {
    let [
        AbstractOperation::SaturatingIntegerSubtract {
            psi_operation,
            result: subtract_result,
            scalar_type,
            left,
            right,
        },
        AbstractOperation::Return {
            psi_edge,
            result,
            value,
            scalar_type: return_type,
            cleanup_actions,
        },
    ] = function.operations.as_slice()
    else {
        return Err(
            StraightLineSaturatingIntegerSubtractParametersTranslationError::SourceOperationRoster,
        );
    };
    if !cleanup_actions.is_empty() {
        return Err(StraightLineSaturatingIntegerSubtractParametersTranslationError::SourceCleanup);
    }
    if *result != envelope.function_result
        || *return_type != ScalarType::Integer(*scalar_type)
        || *value != *subtract_result
    {
        return Err(
            StraightLineSaturatingIntegerSubtractParametersTranslationError::SourceReturnLink,
        );
    }
    if envelope
        .parameters
        .iter()
        .any(|parameter| parameter.value == *subtract_result)
    {
        return Err(
            StraightLineSaturatingIntegerSubtractParametersTranslationError::SourceSubtractResultRoster,
        );
    }
    let (left_parameter_index, left_type) = super::super::parameter(envelope, *left).ok_or(
        StraightLineSaturatingIntegerSubtractParametersTranslationError::SourceLeftOperandLink,
    )?;
    let (right_parameter_index, right_type) = super::super::parameter(envelope, *right).ok_or(
        StraightLineSaturatingIntegerSubtractParametersTranslationError::SourceRightOperandLink,
    )?;
    if left_type != *scalar_type || right_type != *scalar_type {
        return Err(
            StraightLineSaturatingIntegerSubtractParametersTranslationError::SourceOperandTypeMismatch,
        );
    }
    Ok(IntegerArithmeticParametersSource {
        operation: *psi_operation,
        return_edge: *psi_edge,
        source_value: *subtract_result,
        scalar_type: *scalar_type,
        left_value: *left,
        right_value: *right,
        left_parameter_index,
        right_parameter_index,
    })
}
