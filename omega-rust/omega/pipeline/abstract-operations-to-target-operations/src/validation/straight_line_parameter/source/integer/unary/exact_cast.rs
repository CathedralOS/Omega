//! Exact `[IntegerExactCast(parameter), Return]` source replay.

use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::{IntegerType, ScalarType};

use super::super::super::super::model::{
    IntegerExactCastParameterSource, ParameterResultKind, ReconstructedEnvelope,
};
use crate::validation::model::StraightLineIntegerExactCastParameterTranslationError;

pub(in crate::validation::straight_line_parameter) fn is_candidate(
    function: &AbstractFunction,
) -> bool {
    super::super::super::has_candidate_envelope(function, ParameterResultKind::Integer)
        && matches!(
            function.operations.as_slice(),
            [
                AbstractOperation::IntegerExactCast { .. },
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
) -> Result<IntegerExactCastParameterSource, StraightLineIntegerExactCastParameterTranslationError>
{
    let [
        AbstractOperation::IntegerExactCast {
            psi_operation,
            obligation,
            result: cast_result,
            source_type,
            target_type,
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
        return Err(StraightLineIntegerExactCastParameterTranslationError::SourceOperationRoster);
    };
    if !cleanup_actions.is_empty() {
        return Err(StraightLineIntegerExactCastParameterTranslationError::SourceCleanup);
    }
    if *result != envelope.function_result
        || *return_type != ScalarType::Integer(*target_type)
        || *value != *cast_result
    {
        return Err(StraightLineIntegerExactCastParameterTranslationError::SourceReturnLink);
    }
    if envelope
        .parameters
        .iter()
        .any(|parameter| parameter.value == *cast_result)
    {
        return Err(StraightLineIntegerExactCastParameterTranslationError::SourceCastResultRoster);
    }
    let (parameter_index, operand_type) = super::super::parameter(envelope, *operand)
        .ok_or(StraightLineIntegerExactCastParameterTranslationError::SourceOperandLink)?;
    if operand_type != *source_type {
        return Err(
            StraightLineIntegerExactCastParameterTranslationError::SourceOperandTypeMismatch,
        );
    }
    if !is_native(*source_type)
        || !is_native(*target_type)
        || source_type == target_type
        || source_type.can_widen_to(*target_type)
        || !source_type.can_exact_cast_to(*target_type)
    {
        return Err(StraightLineIntegerExactCastParameterTranslationError::SourceCastTypeMismatch);
    }
    Ok(IntegerExactCastParameterSource {
        operation: *psi_operation,
        obligation: *obligation,
        return_edge: *psi_edge,
        source_value: *cast_result,
        source_type: *source_type,
        target_type: *target_type,
        operand_value: *operand,
        parameter_index,
    })
}

fn is_native(integer: IntegerType) -> bool {
    matches!(integer.bits(), 8 | 16 | 32 | 64)
}
