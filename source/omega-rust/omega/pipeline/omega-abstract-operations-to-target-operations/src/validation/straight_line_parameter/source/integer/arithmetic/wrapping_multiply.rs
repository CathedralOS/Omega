//! Exact `[WrappingIntegerMultiply(parameter, parameter), Return]` source replay.

use omega_abstract_operations::{AbstractFunction, AbstractOperation};
use psi_core::ScalarType;

use super::super::super::super::model::{
    IntegerArithmeticParametersSource, ParameterResultKind, ReconstructedEnvelope,
};
use crate::validation::model::StraightLineWrappingIntegerMultiplyParametersTranslationError;

pub(in crate::validation::straight_line_parameter) fn is_candidate(
    function: &AbstractFunction,
) -> bool {
    super::super::super::has_candidate_envelope(function, ParameterResultKind::Integer)
        && matches!(
            function.operations.as_slice(),
            [
                AbstractOperation::WrappingIntegerMultiply { .. },
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
    StraightLineWrappingIntegerMultiplyParametersTranslationError,
> {
    let [
        AbstractOperation::WrappingIntegerMultiply {
            psi_operation,
            result: multiply_result,
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
            StraightLineWrappingIntegerMultiplyParametersTranslationError::SourceOperationRoster,
        );
    };
    if !cleanup_actions.is_empty() {
        return Err(StraightLineWrappingIntegerMultiplyParametersTranslationError::SourceCleanup);
    }
    if *result != envelope.function_result
        || *return_type != ScalarType::Integer(*scalar_type)
        || *value != *multiply_result
    {
        return Err(
            StraightLineWrappingIntegerMultiplyParametersTranslationError::SourceReturnLink,
        );
    }
    if envelope
        .parameters
        .iter()
        .any(|parameter| parameter.value == *multiply_result)
    {
        return Err(
            StraightLineWrappingIntegerMultiplyParametersTranslationError::SourceMultiplyResultRoster,
        );
    }
    let (left_parameter_index, left_type) = super::super::parameter(envelope, *left).ok_or(
        StraightLineWrappingIntegerMultiplyParametersTranslationError::SourceLeftOperandLink,
    )?;
    let (right_parameter_index, right_type) = super::super::parameter(envelope, *right).ok_or(
        StraightLineWrappingIntegerMultiplyParametersTranslationError::SourceRightOperandLink,
    )?;
    if left_type != *scalar_type || right_type != *scalar_type {
        return Err(
            StraightLineWrappingIntegerMultiplyParametersTranslationError::SourceOperandTypeMismatch,
        );
    }
    Ok(IntegerArithmeticParametersSource {
        operation: *psi_operation,
        return_edge: *psi_edge,
        source_value: *multiply_result,
        scalar_type: *scalar_type,
        left_value: *left,
        right_value: *right,
        left_parameter_index,
        right_parameter_index,
    })
}
