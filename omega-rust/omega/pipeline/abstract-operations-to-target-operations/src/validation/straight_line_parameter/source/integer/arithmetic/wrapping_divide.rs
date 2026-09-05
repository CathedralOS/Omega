//! Wrapping `[WrappingIntegerDivide(parameter, parameter), Return]` source replay.
//! Only a nonzero divisor is required; a signed `MIN / -1` quotient wraps.

use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::{IntegerCarrier, ScalarType};

use super::super::super::super::model::{
    IntegerArithmeticParametersSource, ParameterResultKind, ReconstructedEnvelope,
    WrappingIntegerDivideParametersSource,
};
use crate::validation::model::StraightLineWrappingIntegerDivideParametersTranslationError;

pub(in crate::validation::straight_line_parameter) fn is_candidate(
    function: &AbstractFunction,
) -> bool {
    super::super::super::has_candidate_envelope(function, ParameterResultKind::Integer)
        && matches!(
            function.operations.as_slice(),
            [
                AbstractOperation::WrappingIntegerDivide { .. },
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
    WrappingIntegerDivideParametersSource,
    StraightLineWrappingIntegerDivideParametersTranslationError,
> {
    let [
        AbstractOperation::WrappingIntegerDivide {
            psi_operation,
            obligation,
            result: divide_result,
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
            StraightLineWrappingIntegerDivideParametersTranslationError::SourceOperationRoster,
        );
    };
    if !cleanup_actions.is_empty() {
        return Err(StraightLineWrappingIntegerDivideParametersTranslationError::SourceCleanup);
    }
    if *result != envelope.function_result
        || *return_type != ScalarType::Integer(*scalar_type)
        || *value != *divide_result
    {
        return Err(StraightLineWrappingIntegerDivideParametersTranslationError::SourceReturnLink);
    }
    if envelope
        .parameters
        .iter()
        .any(|parameter| parameter.value == *divide_result)
    {
        return Err(
            StraightLineWrappingIntegerDivideParametersTranslationError::SourceDivideResultRoster,
        );
    }
    let (left_parameter_index, left_type) = super::super::parameter(envelope, *left).ok_or(
        StraightLineWrappingIntegerDivideParametersTranslationError::SourceLeftOperandLink,
    )?;
    let (right_parameter_index, right_type) = super::super::parameter(envelope, *right).ok_or(
        StraightLineWrappingIntegerDivideParametersTranslationError::SourceRightOperandLink,
    )?;
    if scalar_type.carrier() != IntegerCarrier::Fixed
        || left_type != *scalar_type
        || right_type != *scalar_type
    {
        return Err(
            StraightLineWrappingIntegerDivideParametersTranslationError::SourceOperandTypeMismatch,
        );
    }
    Ok(WrappingIntegerDivideParametersSource {
        arithmetic: IntegerArithmeticParametersSource {
            operation: *psi_operation,
            return_edge: *psi_edge,
            source_value: *divide_result,
            scalar_type: *scalar_type,
            left_value: *left,
            right_value: *right,
            left_parameter_index,
            right_parameter_index,
        },
        obligation: *obligation,
    })
}
