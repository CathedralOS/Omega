//! Exact `[ExactIntegerMultiply(parameter, parameter), Return]` source replay.

use omega_abstract_operations::{AbstractFunction, AbstractOperation};
use psi_core::{IntegerCarrier, ScalarType};

use super::super::super::super::model::{
    ExactIntegerMultiplyParametersSource, IntegerArithmeticParametersSource, ParameterResultKind,
    ReconstructedEnvelope,
};
use crate::validation::model::StraightLineExactIntegerMultiplyParametersTranslationError;

pub(in crate::validation::straight_line_parameter) fn is_candidate(
    function: &AbstractFunction,
) -> bool {
    super::super::super::has_candidate_envelope(function, ParameterResultKind::Integer)
        && matches!(
            function.operations.as_slice(),
            [
                AbstractOperation::ExactIntegerMultiply { .. },
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
    ExactIntegerMultiplyParametersSource,
    StraightLineExactIntegerMultiplyParametersTranslationError,
> {
    let [
        AbstractOperation::ExactIntegerMultiply {
            psi_operation,
            obligation,
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
            StraightLineExactIntegerMultiplyParametersTranslationError::SourceOperationRoster,
        );
    };
    if !cleanup_actions.is_empty() {
        return Err(StraightLineExactIntegerMultiplyParametersTranslationError::SourceCleanup);
    }
    if *result != envelope.function_result
        || *return_type != ScalarType::Integer(*scalar_type)
        || *value != *multiply_result
    {
        return Err(StraightLineExactIntegerMultiplyParametersTranslationError::SourceReturnLink);
    }
    if envelope
        .parameters
        .iter()
        .any(|parameter| parameter.value == *multiply_result)
    {
        return Err(
            StraightLineExactIntegerMultiplyParametersTranslationError::SourceMultiplyResultRoster,
        );
    }
    let (left_parameter_index, left_type) = super::super::parameter(envelope, *left)
        .ok_or(StraightLineExactIntegerMultiplyParametersTranslationError::SourceLeftOperandLink)?;
    let (right_parameter_index, right_type) = super::super::parameter(envelope, *right).ok_or(
        StraightLineExactIntegerMultiplyParametersTranslationError::SourceRightOperandLink,
    )?;
    if scalar_type.carrier() != IntegerCarrier::Fixed
        || left_type != *scalar_type
        || right_type != *scalar_type
    {
        return Err(
            StraightLineExactIntegerMultiplyParametersTranslationError::SourceOperandTypeMismatch,
        );
    }
    Ok(ExactIntegerMultiplyParametersSource {
        arithmetic: IntegerArithmeticParametersSource {
            operation: *psi_operation,
            return_edge: *psi_edge,
            source_value: *multiply_result,
            scalar_type: *scalar_type,
            left_value: *left,
            right_value: *right,
            left_parameter_index,
            right_parameter_index,
        },
        obligation: *obligation,
    })
}
