//! Derived parameter-expression join, descended by unary, equality, or ordering shape.

pub(super) mod equality;
pub(super) mod ordering;

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;
use psi_core::ScalarType;

use super::abi;
use super::model::ReconstructedBooleanNotParameter;
use super::source;
use crate::validation::model::StraightLineBooleanNotParameterTranslationError;

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
