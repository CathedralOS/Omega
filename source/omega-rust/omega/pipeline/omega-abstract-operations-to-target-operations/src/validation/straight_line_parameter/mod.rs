//! Shared reconstruction join for exact straight-line parameter returns.
//!
//! Result-kind leaves remain distinct catalog families. This entrance owns
//! only their common source-envelope, native-ABI, and provenance replay.

mod abi;
pub(crate) mod boolean;
pub(crate) mod integer;
mod source;

use omega_abstract_operations::{AbstractFunction, AbstractFunctionResult, AbstractOperation};
use omega_target::NativeTarget;
use omega_target_operations::{ScalarParameterLocation, TargetFunction};
use psi_core::{EdgeId, ScalarType, ValueId};

use super::model::StraightLineParameterReconstructionError;

#[derive(Clone, Copy)]
pub(super) enum ParameterResultKind {
    Integer,
    Boolean,
}

pub(super) struct ReconstructedParameterReturn {
    pub(super) return_edge: EdgeId,
    pub(super) source_value: ValueId,
    pub(super) parameter_index: usize,
    pub(super) location: ScalarParameterLocation,
}

pub(super) fn is_candidate(function: &AbstractFunction, result_kind: ParameterResultKind) -> bool {
    let result_matches = match (&function.result, result_kind) {
        (AbstractFunctionResult::Scalar(result), ParameterResultKind::Integer) => {
            matches!(result.scalar_type, ScalarType::Integer(_))
        }
        (AbstractFunctionResult::Scalar(result), ParameterResultKind::Boolean) => {
            result.scalar_type == ScalarType::Boolean
        }
        _ => false,
    };
    !function.parameters.is_empty()
        && function.structural_parameters.is_empty()
        && function.entry_claims.is_empty()
        && function.published_service_ceiling.is_empty()
        && result_matches
        && matches!(
            function.block_entries.as_slice(),
            [entry] if entry.block == function.entry
                && entry.parameters.is_empty()
                && entry.operation_offset == 0
        )
        && matches!(
            function.operations.as_slice(),
            [AbstractOperation::Return {
                cleanup_actions,
                ..
            }] if cleanup_actions.is_empty()
        )
}

pub(super) fn reconstruct_parameter_return(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
    expected_result: ScalarType,
) -> Result<ReconstructedParameterReturn, StraightLineParameterReconstructionError> {
    let reconstructed = source::reconstruct(function, expected_result)?;
    let location = abi::replay(
        &function.parameters,
        reconstructed.parameter_index,
        expected_result,
        expected_target,
    )?;
    if !target.provenance.operations.is_empty()
        || target.provenance.edges.as_slice() != [reconstructed.return_edge]
    {
        return Err(StraightLineParameterReconstructionError::TargetProvenance);
    }
    Ok(ReconstructedParameterReturn {
        return_edge: reconstructed.return_edge,
        source_value: reconstructed.source_value,
        parameter_index: reconstructed.parameter_index,
        location,
    })
}
