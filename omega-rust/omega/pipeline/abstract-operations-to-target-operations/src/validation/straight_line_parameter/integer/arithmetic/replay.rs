//! Shared whole-roster ABI and exact provenance replay for integer arithmetic.

use abstract_operations::AbstractFunction;
use semantic_vocabulary::ScalarType;
use target::NativeTarget;
use target_operations::TargetFunction;

use super::super::super::{
    abi,
    model::{IntegerArithmeticParametersSource, ReconstructedIntegerArithmeticParameters},
};
use crate::validation::model::StraightLineParameterReconstructionError;

pub(super) fn reconstruct<Error>(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
    reconstruct_source: fn(&AbstractFunction) -> Result<IntegerArithmeticParametersSource, Error>,
    target_provenance_error: Error,
) -> Result<ReconstructedIntegerArithmeticParameters, Error>
where
    Error: From<StraightLineParameterReconstructionError>,
{
    let source = reconstruct_source(function)?;
    reconstruct_from_source(
        function,
        expected_target,
        target,
        source,
        target_provenance_error,
    )
}

pub(super) fn reconstruct_from_source<Error>(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
    source: IntegerArithmeticParametersSource,
    target_provenance_error: Error,
) -> Result<ReconstructedIntegerArithmeticParameters, Error>
where
    Error: From<StraightLineParameterReconstructionError>,
{
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
    Ok(ReconstructedIntegerArithmeticParameters {
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
