//! Common source-function envelope for parameter-based scalar families.

use std::collections::BTreeSet;

use abstract_operations::{AbstractFunction, AbstractFunctionResult};
use semantic_vocabulary::ScalarType;

use super::super::model::{ParameterResultKind, ReconstructedEnvelope};
use crate::validation::model::StraightLineParameterReconstructionError;

pub(super) fn is_candidate(function: &AbstractFunction, result_kind: ParameterResultKind) -> bool {
    !function.parameters.is_empty()
        && function.structural_parameters.is_empty()
        && function.entry_claims.is_empty()
        && function.published_service_ceiling.is_empty()
        && result_kind.accepts(&function.result)
        && matches!(
            function.block_entries.as_slice(),
            [entry] if entry.block == function.entry
                && entry.parameters.is_empty()
                && entry.operation_offset == 0
        )
}

pub(super) fn reconstruct(
    function: &AbstractFunction,
    expected_result: ScalarType,
) -> Result<ReconstructedEnvelope<'_>, StraightLineParameterReconstructionError> {
    if function.parameters.is_empty() {
        return Err(StraightLineParameterReconstructionError::SourceParameters);
    }
    if !function.structural_parameters.is_empty() {
        return Err(StraightLineParameterReconstructionError::SourceStructuralParameters);
    }
    let AbstractFunctionResult::Scalar(result) = &function.result else {
        return Err(StraightLineParameterReconstructionError::SourceResult);
    };
    if result.scalar_type != expected_result {
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
    let mut parameter_values = BTreeSet::new();
    if function
        .parameters
        .iter()
        .any(|parameter| !parameter_values.insert(parameter.value))
    {
        return Err(StraightLineParameterReconstructionError::SourceParameterRoster);
    }
    Ok(ReconstructedEnvelope {
        function_result: result.value,
        parameters: &function.parameters,
    })
}
