//! Optimizer module role: executable entrance. Wrapping integer-arithmetic source, ABI, provenance, and target replay.

pub(crate) mod wrapping_add;

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;
use psi_core::ScalarType;

use super::super::{abi, model::ReconstructedWrappingIntegerAddParameters};
use crate::validation::model::StraightLineWrappingIntegerAddParametersTranslationError;

pub(super) fn reconstruct_wrapping_add(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    ReconstructedWrappingIntegerAddParameters,
    StraightLineWrappingIntegerAddParametersTranslationError,
> {
    let source = super::super::source::integer::arithmetic::reconstruct_wrapping_add(function)?;
    let locations = abi::replay(
        &function.parameters,
        ScalarType::Integer(source.scalar_type),
        expected_target,
    )?;
    if target.provenance.operations.as_slice() != [source.operation]
        || target.provenance.edges.as_slice() != [source.return_edge]
    {
        return Err(StraightLineWrappingIntegerAddParametersTranslationError::TargetProvenance);
    }
    Ok(ReconstructedWrappingIntegerAddParameters {
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
