//! Common ABI and provenance replay for integer bitwise expressions.

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;
use psi_core::ScalarType;

use super::super::super::{
    abi,
    model::{IntegerBitwiseParametersSource, ReconstructedIntegerBitwiseParameters},
};
use crate::validation::model::StraightLineParameterReconstructionError;

pub(super) fn reconstruct<Error>(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
    reconstruct_source: fn(&AbstractFunction) -> Result<IntegerBitwiseParametersSource, Error>,
    target_provenance_error: Error,
) -> Result<ReconstructedIntegerBitwiseParameters, Error>
where
    Error: From<StraightLineParameterReconstructionError>,
{
    let source = reconstruct_source(function)?;
    let locations = abi::replay(
        &function.parameters,
        ScalarType::Integer(source.scalar_type),
        expected_target,
    )?;
    if target.provenance.operations.as_slice() != [source.operation]
        || target.provenance.edges.as_slice() != [source.return_edge]
    {
        return Err(target_provenance_error);
    }
    Ok(ReconstructedIntegerBitwiseParameters {
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
