//! Exact `[BooleanEqual(parameter, parameter), Return]` source replay.

use omega_abstract_operations::{AbstractFunction, AbstractOperation};
use psi_core::ScalarType;

use super::super::model::{
    BooleanEqualParametersSource, ParameterResultKind, ReconstructedEnvelope,
};
use crate::validation::model::StraightLineBooleanEqualParametersTranslationError;

pub(in crate::validation::straight_line_parameter) fn is_candidate(
    function: &AbstractFunction,
) -> bool {
    super::has_candidate_envelope(function, ParameterResultKind::Boolean)
        && matches!(
            function.operations.as_slice(),
            [
                AbstractOperation::BooleanEqual { .. },
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
) -> Result<BooleanEqualParametersSource, StraightLineBooleanEqualParametersTranslationError> {
    let [
        AbstractOperation::BooleanEqual {
            psi_operation,
            result: equal_result,
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
        return Err(StraightLineBooleanEqualParametersTranslationError::SourceOperationRoster);
    };
    if !cleanup_actions.is_empty() {
        return Err(StraightLineBooleanEqualParametersTranslationError::SourceCleanup);
    }
    if *result != envelope.function_result
        || *scalar_type != ScalarType::Boolean
        || *value != *equal_result
    {
        return Err(StraightLineBooleanEqualParametersTranslationError::SourceReturnLink);
    }
    if envelope
        .parameters
        .iter()
        .any(|parameter| parameter.value == *equal_result)
    {
        return Err(StraightLineBooleanEqualParametersTranslationError::SourceEqualResultRoster);
    }
    let left_parameter_index = parameter_index(envelope, *left)
        .ok_or(StraightLineBooleanEqualParametersTranslationError::SourceLeftOperandLink)?;
    let right_parameter_index = parameter_index(envelope, *right)
        .ok_or(StraightLineBooleanEqualParametersTranslationError::SourceRightOperandLink)?;
    Ok(BooleanEqualParametersSource {
        equal_operation: *psi_operation,
        return_edge: *psi_edge,
        source_value: *equal_result,
        left_value: *left,
        right_value: *right,
        left_parameter_index,
        right_parameter_index,
    })
}

fn parameter_index(
    envelope: &ReconstructedEnvelope<'_>,
    value: psi_core::ValueId,
) -> Option<usize> {
    envelope.parameters.iter().position(|parameter| {
        parameter.value == value && parameter.scalar_type == ScalarType::Boolean
    })
}
