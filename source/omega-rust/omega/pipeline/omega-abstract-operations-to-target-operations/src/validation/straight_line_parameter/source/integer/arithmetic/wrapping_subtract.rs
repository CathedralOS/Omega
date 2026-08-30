//! Exact `[WrappingIntegerSubtract(parameter, parameter), Return]` source replay.

use omega_abstract_operations::{AbstractFunction, AbstractOperation};
use psi_core::ScalarType;

use super::super::super::super::model::{
    IntegerArithmeticParametersSource, ParameterResultKind, ReconstructedEnvelope,
};
use crate::validation::model::StraightLineWrappingIntegerSubtractParametersTranslationError;

pub(in crate::validation::straight_line_parameter) fn is_candidate(
    function: &AbstractFunction,
) -> bool {
    super::super::super::has_candidate_envelope(function, ParameterResultKind::Integer)
        && matches!(
            function.operations.as_slice(),
            [
                AbstractOperation::WrappingIntegerSubtract { .. },
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
    StraightLineWrappingIntegerSubtractParametersTranslationError,
> {
    let [
        AbstractOperation::WrappingIntegerSubtract {
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
            StraightLineWrappingIntegerSubtractParametersTranslationError::SourceOperationRoster,
        );
    };
    if !cleanup_actions.is_empty() {
        return Err(StraightLineWrappingIntegerSubtractParametersTranslationError::SourceCleanup);
    }
    if *result != envelope.function_result
        || *return_type != ScalarType::Integer(*scalar_type)
        || *value != *subtract_result
    {
        return Err(
            StraightLineWrappingIntegerSubtractParametersTranslationError::SourceReturnLink,
        );
    }
    if envelope
        .parameters
        .iter()
        .any(|parameter| parameter.value == *subtract_result)
    {
        return Err(
            StraightLineWrappingIntegerSubtractParametersTranslationError::SourceSubtractResultRoster,
        );
    }
    let (left_parameter_index, left_type) = super::super::parameter(envelope, *left).ok_or(
        StraightLineWrappingIntegerSubtractParametersTranslationError::SourceLeftOperandLink,
    )?;
    let (right_parameter_index, right_type) = super::super::parameter(envelope, *right).ok_or(
        StraightLineWrappingIntegerSubtractParametersTranslationError::SourceRightOperandLink,
    )?;
    if left_type != *scalar_type || right_type != *scalar_type {
        return Err(
            StraightLineWrappingIntegerSubtractParametersTranslationError::SourceOperandTypeMismatch,
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
