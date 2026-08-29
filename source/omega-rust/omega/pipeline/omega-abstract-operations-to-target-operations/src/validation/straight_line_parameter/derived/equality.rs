//! Whole-roster ABI and provenance replay for typed equality expressions.

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;
use psi_core::ScalarType;

use super::super::abi;
use super::super::model::{
    ReconstructedBooleanEqualParameters, ReconstructedIntegerBinaryBooleanParameters,
};
use super::super::source;
use crate::validation::model::{
    StraightLineBooleanEqualParametersTranslationError,
    StraightLineIntegerEqualParametersTranslationError,
};

pub(in crate::validation::straight_line_parameter) fn reconstruct_boolean_equal(
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

pub(in crate::validation::straight_line_parameter) fn reconstruct_integer_equal(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    ReconstructedIntegerBinaryBooleanParameters,
    StraightLineIntegerEqualParametersTranslationError,
> {
    let source = source::integer::reconstruct_equal(function)?;
    let locations = abi::replay(&function.parameters, ScalarType::Boolean, expected_target)?;
    if target.provenance.operations.as_slice() != [source.operation]
        || target.provenance.edges.as_slice() != [source.return_edge]
    {
        return Err(StraightLineIntegerEqualParametersTranslationError::TargetProvenance);
    }
    Ok(ReconstructedIntegerBinaryBooleanParameters {
        operation: source.operation,
        return_edge: source.return_edge,
        source_value: source.source_value,
        scalar_type: source.scalar_type,
        left_value: source.left_value,
        right_value: source.right_value,
        left_parameter_index: source.left_parameter_index,
        right_parameter_index: source.right_parameter_index,
        left_location: locations[source.left_parameter_index],
        right_location: locations[source.right_parameter_index],
    })
}
