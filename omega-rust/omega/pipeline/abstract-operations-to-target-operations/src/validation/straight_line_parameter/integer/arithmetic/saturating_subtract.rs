//! Exact saturating-integer-subtract-of-parameters target replay.

use abstract_operations::AbstractFunction;
use target::NativeTarget;
use target_operations::{TargetFunction, TargetIntegerExpression, TargetOperation};

use super::super::super::super::{
    StraightLineSaturatingIntegerSubtractParametersTranslationError,
    StraightLineSaturatingIntegerSubtractParametersTranslationReceipt,
};

pub(crate) fn is_candidate(function: &AbstractFunction) -> bool {
    super::super::super::source::integer::arithmetic::saturating_subtract::is_candidate(function)
}

pub(crate) fn validate(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    StraightLineSaturatingIntegerSubtractParametersTranslationReceipt,
    StraightLineSaturatingIntegerSubtractParametersTranslationError,
> {
    let reconstructed = super::reconstruct_saturating_subtract(source, expected_target, target)?;
    let TargetOperation::ReturnIntegerExpression {
        psi_edge,
        source_value,
        scalar_type,
        expression:
            TargetIntegerExpression::SaturatingSubtract {
                psi_operation,
                left,
                right,
            },
    } = &target.operation
    else {
        return Err(
            StraightLineSaturatingIntegerSubtractParametersTranslationError::TargetOperation,
        );
    };
    let TargetIntegerExpression::Parameter {
        source_value: left_value,
        parameter_index: left_parameter_index,
        location: left_location,
    } = left.as_ref()
    else {
        return Err(
            StraightLineSaturatingIntegerSubtractParametersTranslationError::TargetOperation,
        );
    };
    let TargetIntegerExpression::Parameter {
        source_value: right_value,
        parameter_index: right_parameter_index,
        location: right_location,
    } = right.as_ref()
    else {
        return Err(
            StraightLineSaturatingIntegerSubtractParametersTranslationError::TargetOperation,
        );
    };
    if *psi_edge != reconstructed.return_edge
        || *source_value != reconstructed.source_value
        || *scalar_type != reconstructed.scalar_type
        || *psi_operation != reconstructed.operation
        || *left_value != reconstructed.left_value
        || *right_value != reconstructed.right_value
        || *left_parameter_index != reconstructed.left_parameter_index
        || *right_parameter_index != reconstructed.right_parameter_index
        || *left_location != reconstructed.left_location
        || *right_location != reconstructed.right_location
    {
        return Err(
            StraightLineSaturatingIntegerSubtractParametersTranslationError::TargetOperation,
        );
    }
    Ok(
        StraightLineSaturatingIntegerSubtractParametersTranslationReceipt::new(
            source.machine,
            reconstructed.operation,
            reconstructed.return_edge,
            reconstructed.source_value,
            reconstructed.scalar_type,
            reconstructed.left_value,
            reconstructed.right_value,
            reconstructed.left_parameter_index,
            reconstructed.right_parameter_index,
            reconstructed.left_location,
            reconstructed.right_location,
        ),
    )
}
