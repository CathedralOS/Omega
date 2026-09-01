//! Saturating `[SaturatingIntegerRemainder(parameter, parameter), Return]` source replay.
//! Truncation-toward-zero keeps the dividend sign; signed `MIN % -1` is zero.
//! Only the nonzero-divisor obligation is required.

use omega_abstract_operations::{AbstractFunction, AbstractOperation};
use psi_core::{IntegerCarrier, ScalarType};

use super::super::super::super::model::{
    IntegerArithmeticParametersSource, ParameterResultKind, ReconstructedEnvelope,
    SaturatingIntegerRemainderParametersSource,
};
use crate::validation::model::StraightLineSaturatingIntegerRemainderParametersTranslationError;

pub(in crate::validation::straight_line_parameter) fn is_candidate(
    function: &AbstractFunction,
) -> bool {
    super::super::super::has_candidate_envelope(function, ParameterResultKind::Integer)
        && matches!(
            function.operations.as_slice(),
            [
                AbstractOperation::SaturatingIntegerRemainder { .. },
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
    SaturatingIntegerRemainderParametersSource,
    StraightLineSaturatingIntegerRemainderParametersTranslationError,
> {
    let [
        AbstractOperation::SaturatingIntegerRemainder {
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
            StraightLineSaturatingIntegerRemainderParametersTranslationError::SourceOperationRoster,
        );
    };
    if !cleanup_actions.is_empty() {
        return Err(
            StraightLineSaturatingIntegerRemainderParametersTranslationError::SourceCleanup,
        );
    }
    if *result != envelope.function_result
        || *return_type != ScalarType::Integer(*scalar_type)
        || *value != *remainder_result
    {
        return Err(
            StraightLineSaturatingIntegerRemainderParametersTranslationError::SourceReturnLink,
        );
    }
    if envelope
        .parameters
        .iter()
        .any(|parameter| parameter.value == *remainder_result)
    {
        return Err(
            StraightLineSaturatingIntegerRemainderParametersTranslationError::SourceRemainderResultRoster,
        );
    }
    let (left_parameter_index, left_type) = super::super::parameter(envelope, *left).ok_or(
        StraightLineSaturatingIntegerRemainderParametersTranslationError::SourceLeftOperandLink,
    )?;
    let (right_parameter_index, right_type) = super::super::parameter(envelope, *right).ok_or(
        StraightLineSaturatingIntegerRemainderParametersTranslationError::SourceRightOperandLink,
    )?;
    if scalar_type.carrier() != IntegerCarrier::Fixed
        || left_type != *scalar_type
        || right_type != *scalar_type
    {
        return Err(
            StraightLineSaturatingIntegerRemainderParametersTranslationError::SourceOperandTypeMismatch,
        );
    }
    Ok(SaturatingIntegerRemainderParametersSource {
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
