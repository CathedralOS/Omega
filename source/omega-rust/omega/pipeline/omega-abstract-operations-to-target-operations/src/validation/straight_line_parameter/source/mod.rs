//! Source-only reconstruction descended by direct or derived parameter use.

pub(super) mod boolean_not;
pub(super) mod direct;

use std::collections::BTreeSet;

use omega_abstract_operations::{AbstractFunction, AbstractFunctionResult};
use psi_core::ScalarType;

use super::super::StraightLineBooleanNotParameterTranslationError;
use super::{
    StraightLineParameterReconstructionError,
    model::{
        BooleanNotParameterSource, ParameterResultKind, ParameterReturnSource,
        ReconstructedEnvelope,
    },
};

pub(super) fn has_candidate_envelope(
    function: &AbstractFunction,
    result_kind: ParameterResultKind,
) -> bool {
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

pub(super) fn reconstruct_direct(
    function: &AbstractFunction,
    expected_result: ScalarType,
) -> Result<ParameterReturnSource, StraightLineParameterReconstructionError> {
    let envelope = reconstruct_envelope(function, expected_result)?;
    direct::reconstruct(function, &envelope, expected_result)
}

pub(super) fn reconstruct_boolean_not(
    function: &AbstractFunction,
) -> Result<BooleanNotParameterSource, StraightLineBooleanNotParameterTranslationError> {
    let envelope = reconstruct_envelope(function, ScalarType::Boolean)?;
    boolean_not::reconstruct(function, &envelope)
}

fn reconstruct_envelope(
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
