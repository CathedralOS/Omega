//! Shared source-to-ABI-to-provenance join for derived parameter expressions.

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;
use psi_core::ScalarType;

use super::abi;
use super::model::{ReconstructedBooleanEqualParameters, ReconstructedBooleanNotParameter};
use super::source;
use crate::validation::model::{
    StraightLineBooleanEqualParametersTranslationError,
    StraightLineBooleanNotParameterTranslationError,
};

pub(super) fn reconstruct_boolean_not(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<ReconstructedBooleanNotParameter, StraightLineBooleanNotParameterTranslationError> {
    let source = source::reconstruct_boolean_not(function)?;
    let locations = abi::replay(&function.parameters, ScalarType::Boolean, expected_target)?;
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
        location: locations[source.parameter_index],
    })
}

pub(super) fn reconstruct_boolean_equal(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<ReconstructedBooleanEqualParameters, StraightLineBooleanEqualParametersTranslationError>
{
    let source = source::reconstruct_boolean_equal(function)?;
    let locations = abi::replay(&function.parameters, ScalarType::Boolean, expected_target)?;
    if target.provenance.operations.as_slice() != [source.equal_operation]
        || target.provenance.edges.as_slice() != [source.return_edge]
    {
        return Err(StraightLineBooleanEqualParametersTranslationError::TargetProvenance);
    }
    Ok(ReconstructedBooleanEqualParameters {
        equal_operation: source.equal_operation,
        return_edge: source.return_edge,
        source_value: source.source_value,
        left_value: source.left_value,
        right_value: source.right_value,
        left_parameter_index: source.left_parameter_index,
        right_parameter_index: source.right_parameter_index,
        left_location: locations[source.left_parameter_index],
        right_location: locations[source.right_parameter_index],
    })
}
