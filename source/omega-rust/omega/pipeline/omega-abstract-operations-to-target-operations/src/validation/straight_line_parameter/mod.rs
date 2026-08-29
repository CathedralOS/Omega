//! Shared reconstruction join for exact straight-line parameter returns.
//!
//! Result-kind leaves remain distinct catalog families. This entrance owns
//! only their common source-envelope, native-ABI, and provenance replay.

mod abi;
pub(crate) mod boolean;
pub(crate) mod boolean_not;
pub(crate) mod integer;
mod model;
mod source;

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;
use psi_core::ScalarType;

use self::model::{ReconstructedBooleanNotParameter, ReconstructedParameterReturn};
use super::model::{
    StraightLineBooleanNotParameterTranslationError, StraightLineParameterReconstructionError,
};

fn reconstruct_parameter_return(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
    expected_result: ScalarType,
) -> Result<ReconstructedParameterReturn, StraightLineParameterReconstructionError> {
    let source = source::reconstruct_direct(function, expected_result)?;
    let location = abi::replay(
        &function.parameters,
        source.parameter_index,
        expected_result,
        expected_target,
    )?;
    if !target.provenance.operations.is_empty()
        || target.provenance.edges.as_slice() != [source.return_edge]
    {
        return Err(StraightLineParameterReconstructionError::TargetProvenance);
    }
    Ok(ReconstructedParameterReturn {
        return_edge: source.return_edge,
        source_value: source.source_value,
        parameter_index: source.parameter_index,
        location,
    })
}

fn reconstruct_boolean_not_parameter(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<ReconstructedBooleanNotParameter, StraightLineBooleanNotParameterTranslationError> {
    let source = source::reconstruct_boolean_not(function)?;
    let location = abi::replay(
        &function.parameters,
        source.parameter_index,
        ScalarType::Boolean,
        expected_target,
    )?;
    if target.provenance.operations.as_slice() != [source.not_operation]
        || target.provenance.edges.as_slice() != [source.return_edge]
    {
        return Err(StraightLineBooleanNotParameterTranslationError::TargetProvenance);
    }
    Ok(ReconstructedBooleanNotParameter {
        not_operation: source.not_operation,
        return_edge: source.return_edge,
        source_value: source.source_value,
        operand_value: source.operand_value,
        parameter_index: source.parameter_index,
        location,
    })
}
