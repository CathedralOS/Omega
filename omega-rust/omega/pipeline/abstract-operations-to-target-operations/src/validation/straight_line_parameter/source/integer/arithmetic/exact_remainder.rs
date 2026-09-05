//! Exact `[ExactIntegerRemainder(parameter, parameter), Return]` source replay.

use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::{IntegerCarrier, ScalarType};

use super::super::super::super::model::{
    ExactIntegerRemainderParametersSource, IntegerArithmeticParametersSource, ParameterResultKind,
    ReconstructedEnvelope,
};
use crate::validation::model::StraightLineExactIntegerRemainderParametersTranslationError;

pub(in crate::validation::straight_line_parameter) fn is_candidate(
    function: &AbstractFunction,
) -> bool {
    super::super::super::has_candidate_envelope(function, ParameterResultKind::Integer)
        && matches!(
            function.operations.as_slice(),
            [
                AbstractOperation::ExactIntegerRemainder { .. },
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
    ExactIntegerRemainderParametersSource,
    StraightLineExactIntegerRemainderParametersTranslationError,
> {
    let [
        AbstractOperation::ExactIntegerRemainder {
            psi_operation,
            obligation,
            result: remainder_result,
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
            StraightLineExactIntegerRemainderParametersTranslationError::SourceOperationRoster,
        );
    };
    if !cleanup_actions.is_empty() {
        return Err(StraightLineExactIntegerRemainderParametersTranslationError::SourceCleanup);
    }
    if *result != envelope.function_result
        || *return_type != ScalarType::Integer(*scalar_type)
        || *value != *remainder_result
    {
        return Err(StraightLineExactIntegerRemainderParametersTranslationError::SourceReturnLink);
    }
    if envelope
        .parameters
        .iter()
        .any(|parameter| parameter.value == *remainder_result)
    {
        return Err(
            StraightLineExactIntegerRemainderParametersTranslationError::SourceRemainderResultRoster,
        );
    }
    let (left_parameter_index, left_type) = super::super::parameter(envelope, *left).ok_or(
        StraightLineExactIntegerRemainderParametersTranslationError::SourceLeftOperandLink,
    )?;
    let (right_parameter_index, right_type) = super::super::parameter(envelope, *right).ok_or(
        StraightLineExactIntegerRemainderParametersTranslationError::SourceRightOperandLink,
    )?;
    if scalar_type.carrier() != IntegerCarrier::Fixed
        || left_type != *scalar_type
        || right_type != *scalar_type
    {
        return Err(
            StraightLineExactIntegerRemainderParametersTranslationError::SourceOperandTypeMismatch,
        );
    }
    Ok(ExactIntegerRemainderParametersSource {
        arithmetic: IntegerArithmeticParametersSource {
            operation: *psi_operation,
            return_edge: *psi_edge,
            source_value: *remainder_result,
            scalar_type: *scalar_type,
            left_value: *left,
            right_value: *right,
            left_parameter_index,
            right_parameter_index,
        },
        obligation: *obligation,
    })
}
