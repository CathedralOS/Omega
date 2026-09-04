//! Exact `[IntegerBitwiseOr(parameter, parameter), Return]` source replay.

use omega_abstract_operations::{AbstractFunction, AbstractOperation};
use psi_core::{IntegerCarrier, ScalarType};

use super::super::super::super::model::{
    IntegerBitwiseParametersSource, ParameterResultKind, ReconstructedEnvelope,
};
use crate::validation::model::StraightLineIntegerBitwiseOrParametersTranslationError;

pub(in crate::validation::straight_line_parameter) fn is_candidate(
    function: &AbstractFunction,
) -> bool {
    super::super::super::has_candidate_envelope(function, ParameterResultKind::Integer)
        && matches!(
            function.operations.as_slice(),
            [
                AbstractOperation::IntegerBitwiseOr { .. },
                AbstractOperation::Return { cleanup_actions, .. }
            ] if cleanup_actions.is_empty()
        )
}

pub(super) fn reconstruct(
    function: &AbstractFunction,
    envelope: &ReconstructedEnvelope<'_>,
) -> Result<IntegerBitwiseParametersSource, StraightLineIntegerBitwiseOrParametersTranslationError>
{
    let [
        AbstractOperation::IntegerBitwiseOr {
            psi_operation,
            result: or_result,
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
        return Err(StraightLineIntegerBitwiseOrParametersTranslationError::SourceOperationRoster);
    };
    if !cleanup_actions.is_empty() {
        return Err(StraightLineIntegerBitwiseOrParametersTranslationError::SourceCleanup);
    }
    if *result != envelope.function_result
        || *return_type != ScalarType::Integer(*scalar_type)
        || *value != *or_result
    {
        return Err(StraightLineIntegerBitwiseOrParametersTranslationError::SourceReturnLink);
    }
    if envelope
        .parameters
        .iter()
        .any(|parameter| parameter.value == *or_result)
    {
        return Err(StraightLineIntegerBitwiseOrParametersTranslationError::SourceOrResultRoster);
    }
    let (left_parameter_index, left_type) = super::super::parameter(envelope, *left)
        .ok_or(StraightLineIntegerBitwiseOrParametersTranslationError::SourceLeftOperandLink)?;
    let (right_parameter_index, right_type) = super::super::parameter(envelope, *right)
        .ok_or(StraightLineIntegerBitwiseOrParametersTranslationError::SourceRightOperandLink)?;
    if left_type != *scalar_type || right_type != *scalar_type {
        return Err(
            StraightLineIntegerBitwiseOrParametersTranslationError::SourceOperandTypeMismatch,
        );
    }
    if scalar_type.carrier() != IntegerCarrier::Fixed
        || !matches!(scalar_type.bits(), 8 | 16 | 32 | 64)
    {
        return Err(StraightLineIntegerBitwiseOrParametersTranslationError::SourceOrTypeMismatch);
    }
    Ok(IntegerBitwiseParametersSource {
        operation: *psi_operation,
        return_edge: *psi_edge,
        source_value: *or_result,
        scalar_type: *scalar_type,
        left_value: *left,
        right_value: *right,
        left_parameter_index,
        right_parameter_index,
    })
}
