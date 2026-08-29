//! Whole-roster ABI and provenance replay for typed ordering expressions.

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;
use psi_core::ScalarType;

use super::super::abi;
use super::super::model::ReconstructedIntegerBinaryBooleanParameters;
use super::super::source;
use crate::validation::model::StraightLineIntegerLessThanParametersTranslationError;

pub(in crate::validation::straight_line_parameter) fn reconstruct_integer_less_than(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    ReconstructedIntegerBinaryBooleanParameters,
    StraightLineIntegerLessThanParametersTranslationError,
> {
    let source = source::reconstruct_integer_less_than(function)?;
    let locations = abi::replay(&function.parameters, ScalarType::Boolean, expected_target)?;
    if target.provenance.operations.as_slice() != [source.operation]
        || target.provenance.edges.as_slice() != [source.return_edge]
    {
        return Err(StraightLineIntegerLessThanParametersTranslationError::TargetProvenance);
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
