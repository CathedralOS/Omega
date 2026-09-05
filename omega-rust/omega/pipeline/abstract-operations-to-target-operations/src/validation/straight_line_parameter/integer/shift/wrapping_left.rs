//! Wrapping shift-left target replay with independent value/count type custody.

use abstract_operations::AbstractFunction;
use target::NativeTarget;
use target_operations::{TargetFunction, TargetIntegerExpression, TargetOperation};

use super::super::super::super::{
    StraightLineWrappingIntegerShiftLeftParametersTranslationError,
    StraightLineWrappingIntegerShiftLeftParametersTranslationReceipt,
};

pub(crate) fn is_candidate(function: &AbstractFunction) -> bool {
    super::super::super::source::integer::shift::wrapping_left::is_candidate(function)
}

pub(crate) fn validate(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    StraightLineWrappingIntegerShiftLeftParametersTranslationReceipt,
    StraightLineWrappingIntegerShiftLeftParametersTranslationError,
> {
    let reconstructed = super::reconstruct_wrapping_left(source, expected_target, target)?;
    let TargetOperation::ReturnIntegerExpression {
        psi_edge,
        source_value,
        scalar_type,
        expression:
            TargetIntegerExpression::WrappingShiftLeft {
                psi_operation,
                count_type,
                value,
                count,
            },
    } = &target.operation
    else {
        return Err(
            StraightLineWrappingIntegerShiftLeftParametersTranslationError::TargetOperation,
        );
    };
    let TargetIntegerExpression::Parameter {
        source_value: value_id,
        parameter_index: value_parameter_index,
        location: value_location,
    } = value.as_ref()
    else {
        return Err(
            StraightLineWrappingIntegerShiftLeftParametersTranslationError::TargetOperation,
        );
    };
    let TargetIntegerExpression::Parameter {
        source_value: count_id,
        parameter_index: count_parameter_index,
        location: count_location,
    } = count.as_ref()
    else {
        return Err(
            StraightLineWrappingIntegerShiftLeftParametersTranslationError::TargetOperation,
        );
    };
    if *psi_edge != reconstructed.return_edge
        || *source_value != reconstructed.source_value
        || *scalar_type != reconstructed.value_type
        || *count_type != reconstructed.count_type
        || *psi_operation != reconstructed.operation
        || *value_id != reconstructed.value
        || *count_id != reconstructed.count
        || *value_parameter_index != reconstructed.value_parameter_index
        || *count_parameter_index != reconstructed.count_parameter_index
        || *value_location != reconstructed.value_location
        || *count_location != reconstructed.count_location
    {
        return Err(
            StraightLineWrappingIntegerShiftLeftParametersTranslationError::TargetOperation,
        );
    }
    Ok(
        StraightLineWrappingIntegerShiftLeftParametersTranslationReceipt::new(
            source.machine,
            reconstructed.operation,
            reconstructed.return_edge,
            reconstructed.source_value,
            reconstructed.value_type,
            reconstructed.count_type,
            reconstructed.value,
            reconstructed.count,
            reconstructed.value_parameter_index,
            reconstructed.count_parameter_index,
            reconstructed.value_location,
            reconstructed.count_location,
        ),
    )
}
