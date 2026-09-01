//! Whole-roster ABI and provenance reconstruction for integer shifts.

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::TargetFunction;
use psi_core::ScalarType;

use super::super::super::{
    abi,
    model::{IntegerShiftParametersSource, ReconstructedIntegerShiftParameters},
};
use crate::validation::model::{
    StraightLineParameterReconstructionError,
    StraightLineWrappingIntegerShiftLeftParametersTranslationError,
    StraightLineWrappingIntegerShiftRightParametersTranslationError,
};

pub(in crate::validation::straight_line_parameter) fn reconstruct_wrapping_left(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    ReconstructedIntegerShiftParameters,
    StraightLineWrappingIntegerShiftLeftParametersTranslationError,
> {
    let source = super::super::super::source::integer::shift::reconstruct_wrapping_left(function)?;
    reconstruct(
        function,
        expected_target,
        target,
        source,
        StraightLineWrappingIntegerShiftLeftParametersTranslationError::TargetProvenance,
    )
}

pub(in crate::validation::straight_line_parameter) fn reconstruct_wrapping_right(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    ReconstructedIntegerShiftParameters,
    StraightLineWrappingIntegerShiftRightParametersTranslationError,
> {
    let source = super::super::super::source::integer::shift::reconstruct_wrapping_right(function)?;
    reconstruct(
        function,
        expected_target,
        target,
        source,
        StraightLineWrappingIntegerShiftRightParametersTranslationError::TargetProvenance,
    )
}

fn reconstruct<Error>(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
    source: IntegerShiftParametersSource,
    target_provenance_error: Error,
) -> Result<ReconstructedIntegerShiftParameters, Error>
where
    Error: From<StraightLineParameterReconstructionError>,
{
    let locations = abi::replay(
        &function.parameters,
        ScalarType::Integer(source.value_type),
        expected_target,
    )?;
    if target.provenance.operations.as_slice() != [source.operation]
        || target.provenance.edges.as_slice() != [source.return_edge]
    {
        return Err(target_provenance_error);
    }
    Ok(ReconstructedIntegerShiftParameters {
        operation: source.operation,
        return_edge: source.return_edge,
        source_value: source.source_value,
        value_type: source.value_type,
        count_type: source.count_type,
        value: source.value,
        count: source.count,
        value_parameter_index: source.value_parameter_index,
        count_parameter_index: source.count_parameter_index,
        value_location: locations[source.value_parameter_index],
        count_location: locations[source.count_parameter_index],
    })
}
