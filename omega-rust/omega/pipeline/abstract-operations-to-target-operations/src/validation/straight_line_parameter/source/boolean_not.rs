use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::ScalarType;

use super::super::model::{BooleanNotParameterSource, ParameterResultKind, ReconstructedEnvelope};
use crate::validation::model::StraightLineBooleanNotParameterTranslationError;

pub(in crate::validation::straight_line_parameter) fn is_candidate(
    function: &AbstractFunction,
) -> bool {
    super::has_candidate_envelope(function, ParameterResultKind::Boolean)
        && matches!(
            function.operations.as_slice(),
            [
                AbstractOperation::BooleanNot { .. },
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
) -> Result<BooleanNotParameterSource, StraightLineBooleanNotParameterTranslationError> {
    let [
        AbstractOperation::BooleanNot {
            psi_operation,
            result: not_result,
            operand,
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
        return Err(StraightLineBooleanNotParameterTranslationError::SourceOperationRoster);
    };
    if !cleanup_actions.is_empty() {
        return Err(StraightLineBooleanNotParameterTranslationError::SourceCleanup);
    }
    if *result != envelope.function_result
        || *scalar_type != ScalarType::Boolean
        || *value != *not_result
    {
        return Err(StraightLineBooleanNotParameterTranslationError::SourceReturnLink);
    }
    if envelope
        .parameters
        .iter()
        .any(|parameter| parameter.value == *not_result)
    {
        return Err(StraightLineBooleanNotParameterTranslationError::SourceNotResultRoster);
    }
    let Some(parameter_index) = envelope
        .parameters
        .iter()
        .position(|parameter| parameter.value == *operand)
    else {
        return Err(StraightLineBooleanNotParameterTranslationError::SourceOperandLink);
    };
    if envelope.parameters[parameter_index].scalar_type != ScalarType::Boolean {
        return Err(StraightLineBooleanNotParameterTranslationError::SourceOperandLink);
    }
    Ok(BooleanNotParameterSource {
        not_operation: *psi_operation,
        return_edge: *psi_edge,
        source_value: *not_result,
        operand_value: *operand,
        parameter_index,
    })
}
