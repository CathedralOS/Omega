//! Derived parameter-expression join, descended by unary, equality, or ordering shape.

pub(super) mod equality;
pub(super) mod ordering;

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;
use psi_core::ScalarType;

use super::abi;
use super::model::{ReconstructedBooleanNotParameter, ReconstructedIntegerUnaryParameter};
use super::source;
use crate::validation::model::{
    StraightLineBooleanNotParameterTranslationError,
    StraightLineIntegerBitwiseNotParameterTranslationError,
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

pub(super) fn reconstruct_integer_bitwise_not(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    ReconstructedIntegerUnaryParameter,
    StraightLineIntegerBitwiseNotParameterTranslationError,
> {
    let source = source::integer::reconstruct_bitwise_not(function)?;
    let locations = abi::replay(
        &function.parameters,
        ScalarType::Integer(source.scalar_type),
        expected_target,
    )?;
    if target.provenance.operations.as_slice() != [source.operation]
        || target.provenance.edges.as_slice() != [source.return_edge]
    {
        return Err(StraightLineIntegerBitwiseNotParameterTranslationError::TargetProvenance);
    }
    Ok(ReconstructedIntegerUnaryParameter {
        operation: source.operation,
        return_edge: source.return_edge,
        source_value: source.source_value,
        scalar_type: source.scalar_type,
        operand_value: source.operand_value,
        parameter_index: source.parameter_index,
        location: locations[source.parameter_index],
    })
}
