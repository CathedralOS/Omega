use std::collections::BTreeSet;

use omega_abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractResult,
};
use psi_core::{EdgeId, ScalarType, ValueId};

use super::super::model::StraightLineParameterReconstructionError;

pub(super) struct ReconstructedSource {
    pub(super) return_edge: EdgeId,
    pub(super) source_value: ValueId,
    pub(super) parameter_index: usize,
}

pub(super) fn reconstruct(
    function: &AbstractFunction,
    expected_result: ScalarType,
) -> Result<ReconstructedSource, StraightLineParameterReconstructionError> {
    if function.parameters.is_empty() {
        return Err(StraightLineParameterReconstructionError::SourceParameters);
    }
    if !function.structural_parameters.is_empty() {
        return Err(StraightLineParameterReconstructionError::SourceStructuralParameters);
    }
    let AbstractFunctionResult::Scalar(AbstractResult {
        value: function_result,
        scalar_type,
    }) = function.result
    else {
        return Err(StraightLineParameterReconstructionError::SourceResult);
    };
    if scalar_type != expected_result {
        return Err(StraightLineParameterReconstructionError::SourceResult);
    }
    if !function.entry_claims.is_empty() {
        return Err(StraightLineParameterReconstructionError::SourceEntryClaims);
    }
    if !function.published_service_ceiling.is_empty() {
        return Err(StraightLineParameterReconstructionError::SourcePublishedServices);
    }
    if !matches!(
        function.block_entries.as_slice(),
        [entry] if entry.block == function.entry
            && entry.parameters.is_empty()
            && entry.operation_offset == 0
    ) {
        return Err(StraightLineParameterReconstructionError::SourceBlockRoster);
    }
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
    if *result != function_result || *scalar_type != expected_result {
        return Err(StraightLineParameterReconstructionError::SourceReturnLink);
    }

    let mut parameter_values = BTreeSet::new();
    if function
        .parameters
        .iter()
        .any(|parameter| !parameter_values.insert(parameter.value))
    {
        return Err(StraightLineParameterReconstructionError::SourceParameterRoster);
    }
    let Some(parameter_index) = function
        .parameters
        .iter()
        .position(|parameter| parameter.value == *value)
    else {
        return Err(StraightLineParameterReconstructionError::SourceReturnLink);
    };
    if function.parameters[parameter_index].scalar_type != expected_result {
        return Err(StraightLineParameterReconstructionError::SourceReturnLink);
    }

    Ok(ReconstructedSource {
        return_edge: *psi_edge,
        source_value: *value,
        parameter_index,
    })
}
