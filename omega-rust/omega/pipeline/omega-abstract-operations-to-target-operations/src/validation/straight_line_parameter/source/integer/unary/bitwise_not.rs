//! Exact `[IntegerBitwiseNot(parameter), Return]` source replay.

use omega_abstract_operations::{AbstractFunction, AbstractOperation};
use psi_core::ScalarType;

use super::super::super::super::model::{
    IntegerUnaryParameterSource, ParameterResultKind, ReconstructedEnvelope,
};
use crate::validation::model::StraightLineIntegerBitwiseNotParameterTranslationError;

pub(in crate::validation::straight_line_parameter) fn is_candidate(
    function: &AbstractFunction,
) -> bool {
    super::super::super::has_candidate_envelope(function, ParameterResultKind::Integer)
        && matches!(
            function.operations.as_slice(),
            [
                AbstractOperation::IntegerBitwiseNot { .. },
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
) -> Result<IntegerUnaryParameterSource, StraightLineIntegerBitwiseNotParameterTranslationError> {
    let [
        AbstractOperation::IntegerBitwiseNot {
            psi_operation,
            result: bitwise_not_result,
            scalar_type,
            operand,
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
        return Err(StraightLineIntegerBitwiseNotParameterTranslationError::SourceOperationRoster);
    };
    if !cleanup_actions.is_empty() {
        return Err(StraightLineIntegerBitwiseNotParameterTranslationError::SourceCleanup);
    }
    if *result != envelope.function_result
        || *return_type != ScalarType::Integer(*scalar_type)
        || *value != *bitwise_not_result
    {
        return Err(StraightLineIntegerBitwiseNotParameterTranslationError::SourceReturnLink);
    }
    if envelope
        .parameters
        .iter()
        .any(|parameter| parameter.value == *bitwise_not_result)
    {
        return Err(
            StraightLineIntegerBitwiseNotParameterTranslationError::SourceBitwiseNotResultRoster,
        );
    }
    let (parameter_index, operand_type) = super::super::parameter(envelope, *operand)
        .ok_or(StraightLineIntegerBitwiseNotParameterTranslationError::SourceOperandLink)?;
    if operand_type != *scalar_type {
        return Err(
            StraightLineIntegerBitwiseNotParameterTranslationError::SourceOperandTypeMismatch,
        );
    }
    Ok(IntegerUnaryParameterSource {
        operation: *psi_operation,
        return_edge: *psi_edge,
        source_value: *bitwise_not_result,
        scalar_type: *scalar_type,
        operand_value: *operand,
        parameter_index,
    })
}
