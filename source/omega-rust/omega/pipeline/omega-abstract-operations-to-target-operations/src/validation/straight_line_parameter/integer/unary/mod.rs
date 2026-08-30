//! Integer-unary source, ABI, provenance, and typed-target replay entrance.

pub(crate) mod bitwise_not;
pub(crate) mod widen;

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;
use psi_core::ScalarType;

use super::super::{
    abi,
    model::{ReconstructedIntegerUnaryParameter, ReconstructedIntegerWidenParameter},
    source,
};
use crate::validation::model::{
    StraightLineIntegerBitwiseNotParameterTranslationError,
    StraightLineIntegerWidenParameterTranslationError,
};

pub(super) fn reconstruct_bitwise_not(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    ReconstructedIntegerUnaryParameter,
    StraightLineIntegerBitwiseNotParameterTranslationError,
> {
    let source = source::integer::unary::reconstruct_bitwise_not(function)?;
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

pub(super) fn reconstruct_widen(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<ReconstructedIntegerWidenParameter, StraightLineIntegerWidenParameterTranslationError> {
    let source = source::integer::unary::reconstruct_widen(function)?;
    let locations = abi::replay(
        &function.parameters,
        ScalarType::Integer(source.target_type),
        expected_target,
    )?;
    if target.provenance.operations.as_slice() != [source.operation]
        || target.provenance.edges.as_slice() != [source.return_edge]
    {
        return Err(StraightLineIntegerWidenParameterTranslationError::TargetProvenance);
    }
    Ok(ReconstructedIntegerWidenParameter {
        operation: source.operation,
        return_edge: source.return_edge,
        source_value: source.source_value,
        source_type: source.source_type,
        target_type: source.target_type,
        operand_value: source.operand_value,
        parameter_index: source.parameter_index,
        location: locations[source.parameter_index],
    })
}
