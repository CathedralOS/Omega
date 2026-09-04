use omega_abstract_operations::{AbstractFunction, AbstractOperation};
use psi_core::ScalarType;

use super::super::model::ReconstructedEnvelope;
use super::super::{
    StraightLineParameterReconstructionError,
    model::{ParameterResultKind, ParameterReturnSource},
};

pub(in crate::validation::straight_line_parameter) fn is_candidate(
    function: &AbstractFunction,
    result_kind: ParameterResultKind,
) -> bool {
    super::has_candidate_envelope(function, result_kind)
        && matches!(
            function.operations.as_slice(),
            [AbstractOperation::Return {
                cleanup_actions,
                ..
            }] if cleanup_actions.is_empty()
        )
}

pub(super) fn reconstruct(
    function: &AbstractFunction,
    envelope: &ReconstructedEnvelope<'_>,
    expected_result: ScalarType,
) -> Result<ParameterReturnSource, StraightLineParameterReconstructionError> {
    let [
        AbstractOperation::Return {
            psi_edge,
            result,
            value,
            scalar_type,
            cleanup_actions,
        },
    ] = function.operations.as_slice()
    else {
        return Err(StraightLineParameterReconstructionError::SourceOperationRoster);
    };
    if !cleanup_actions.is_empty() {
        return Err(StraightLineParameterReconstructionError::SourceCleanup);
    }
    if *result != envelope.function_result || *scalar_type != expected_result {
        return Err(StraightLineParameterReconstructionError::SourceReturnLink);
    }
    let Some(parameter_index) = envelope
        .parameters
        .iter()
        .position(|parameter| parameter.value == *value)
    else {
        return Err(StraightLineParameterReconstructionError::SourceReturnLink);
    };
    if envelope.parameters[parameter_index].scalar_type != expected_result {
        return Err(StraightLineParameterReconstructionError::SourceReturnLink);
    }
    Ok(ParameterReturnSource {
        return_edge: *psi_edge,
        source_value: *value,
        parameter_index,
    })
}
