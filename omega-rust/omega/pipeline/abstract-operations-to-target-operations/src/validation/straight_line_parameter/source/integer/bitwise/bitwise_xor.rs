//! Exact `[IntegerBitwiseXor(parameter, parameter), Return]` source replay.

use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::{IntegerCarrier, ScalarType};

use super::super::super::super::model::{
    IntegerBitwiseParametersSource, ParameterResultKind, ReconstructedEnvelope,
};
use crate::validation::model::StraightLineIntegerBitwiseXorParametersTranslationError;

pub(in crate::validation::straight_line_parameter) fn is_candidate(
    function: &AbstractFunction,
) -> bool {
    super::super::super::has_candidate_envelope(function, ParameterResultKind::Integer)
        && matches!(
            function.operations.as_slice(),
            [
                AbstractOperation::IntegerBitwiseXor { .. },
                AbstractOperation::Return { cleanup_actions, .. }
            ] if cleanup_actions.is_empty()
        )
}

pub(super) fn reconstruct(
    function: &AbstractFunction,
    envelope: &ReconstructedEnvelope<'_>,
) -> Result<IntegerBitwiseParametersSource, StraightLineIntegerBitwiseXorParametersTranslationError>
{
    let [
        AbstractOperation::IntegerBitwiseXor {
            psi_operation,
            result: xor_result,
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
        return Err(StraightLineIntegerBitwiseXorParametersTranslationError::SourceOperationRoster);
    };
    if !cleanup_actions.is_empty() {
        return Err(StraightLineIntegerBitwiseXorParametersTranslationError::SourceCleanup);
    }
    if *result != envelope.function_result
        || *return_type != ScalarType::Integer(*scalar_type)
        || *value != *xor_result
    {
        return Err(StraightLineIntegerBitwiseXorParametersTranslationError::SourceReturnLink);
    }
    if envelope
        .parameters
        .iter()
        .any(|parameter| parameter.value == *xor_result)
    {
        return Err(StraightLineIntegerBitwiseXorParametersTranslationError::SourceXorResultRoster);
    }
    let (left_parameter_index, left_type) = super::super::parameter(envelope, *left)
        .ok_or(StraightLineIntegerBitwiseXorParametersTranslationError::SourceLeftOperandLink)?;
    let (right_parameter_index, right_type) = super::super::parameter(envelope, *right)
        .ok_or(StraightLineIntegerBitwiseXorParametersTranslationError::SourceRightOperandLink)?;
    if left_type != *scalar_type || right_type != *scalar_type {
        return Err(
            StraightLineIntegerBitwiseXorParametersTranslationError::SourceOperandTypeMismatch,
        );
    }
    if scalar_type.carrier() != IntegerCarrier::Fixed
        || !matches!(scalar_type.bits(), 8 | 16 | 32 | 64)
    {
        return Err(StraightLineIntegerBitwiseXorParametersTranslationError::SourceXorTypeMismatch);
    }
    Ok(IntegerBitwiseParametersSource {
        operation: *psi_operation,
        return_edge: *psi_edge,
        source_value: *xor_result,
        scalar_type: *scalar_type,
        left_value: *left,
        right_value: *right,
        left_parameter_index,
        right_parameter_index,
    })
}
