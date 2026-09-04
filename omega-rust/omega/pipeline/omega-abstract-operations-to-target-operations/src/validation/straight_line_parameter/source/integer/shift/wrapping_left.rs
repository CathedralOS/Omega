//! Wrapping `[WrappingIntegerShiftLeft(parameter, parameter), Return]` source replay.
//! Value and count retain independent ABI-legal integer types.

use omega_abstract_operations::{AbstractFunction, AbstractOperation};
use psi_core::ScalarType;

use super::super::super::super::model::{
    IntegerShiftParametersSource, ParameterResultKind, ReconstructedEnvelope,
};
use crate::validation::model::StraightLineWrappingIntegerShiftLeftParametersTranslationError;

pub(in crate::validation::straight_line_parameter) fn is_candidate(
    function: &AbstractFunction,
) -> bool {
    super::super::super::has_candidate_envelope(function, ParameterResultKind::Integer)
        && matches!(
            function.operations.as_slice(),
            [
                AbstractOperation::WrappingIntegerShiftLeft { .. },
                AbstractOperation::Return { cleanup_actions, .. }
            ] if cleanup_actions.is_empty()
        )
}

pub(super) fn reconstruct(
    function: &AbstractFunction,
    envelope: &ReconstructedEnvelope<'_>,
) -> Result<
    IntegerShiftParametersSource,
    StraightLineWrappingIntegerShiftLeftParametersTranslationError,
> {
    let [
        AbstractOperation::WrappingIntegerShiftLeft {
            psi_operation,
            result: shift_result,
            value_type,
            count_type,
            value,
            count,
        },
        AbstractOperation::Return {
            psi_edge,
            result,
            value: returned_value,
            scalar_type: return_type,
            cleanup_actions,
        },
    ] = function.operations.as_slice()
    else {
        return Err(
            StraightLineWrappingIntegerShiftLeftParametersTranslationError::SourceOperationRoster,
        );
    };
    if !cleanup_actions.is_empty() {
        return Err(StraightLineWrappingIntegerShiftLeftParametersTranslationError::SourceCleanup);
    }
    if *result != envelope.function_result
        || *return_type != ScalarType::Integer(*value_type)
        || *returned_value != *shift_result
    {
        return Err(
            StraightLineWrappingIntegerShiftLeftParametersTranslationError::SourceReturnLink,
        );
    }
    if envelope
        .parameters
        .iter()
        .any(|parameter| parameter.value == *shift_result)
    {
        return Err(
            StraightLineWrappingIntegerShiftLeftParametersTranslationError::SourceShiftResultRoster,
        );
    }
    let (value_parameter_index, parameter_value_type) = super::super::parameter(envelope, *value)
        .ok_or(
        StraightLineWrappingIntegerShiftLeftParametersTranslationError::SourceValueOperandLink,
    )?;
    let (count_parameter_index, parameter_count_type) = super::super::parameter(envelope, *count)
        .ok_or(
        StraightLineWrappingIntegerShiftLeftParametersTranslationError::SourceCountOperandLink,
    )?;
    if parameter_value_type != *value_type {
        return Err(
            StraightLineWrappingIntegerShiftLeftParametersTranslationError::SourceValueTypeMismatch,
        );
    }
    if parameter_count_type != *count_type {
        return Err(
            StraightLineWrappingIntegerShiftLeftParametersTranslationError::SourceCountTypeMismatch,
        );
    }
    Ok(IntegerShiftParametersSource {
        operation: *psi_operation,
        return_edge: *psi_edge,
        source_value: *shift_result,
        value_type: *value_type,
        count_type: *count_type,
        value: *value,
        count: *count,
        value_parameter_index,
        count_parameter_index,
    })
}
