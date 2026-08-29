use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::{TargetBooleanExpression, TargetFunction, TargetOperation};

use super::super::{
    StraightLineBooleanEqualParametersTranslationError,
    StraightLineBooleanEqualParametersTranslationReceipt,
};
use super::source;

pub(crate) fn is_candidate(function: &AbstractFunction) -> bool {
    source::boolean_equal::is_candidate(function)
}

pub(crate) fn validate(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    StraightLineBooleanEqualParametersTranslationReceipt,
    StraightLineBooleanEqualParametersTranslationError,
> {
    let reconstructed = super::derived::reconstruct_boolean_equal(source, expected_target, target)?;
    let TargetOperation::ReturnBooleanExpression {
        psi_edge,
        source_value,
        expression:
            TargetBooleanExpression::Equal {
                psi_operation,
                left,
                right,
            },
    } = &target.operation
    else {
        return Err(StraightLineBooleanEqualParametersTranslationError::TargetOperation);
    };
    let TargetBooleanExpression::Parameter {
        source_value: left_value,
        parameter_index: left_parameter_index,
        location: left_location,
    } = left.as_ref()
    else {
        return Err(StraightLineBooleanEqualParametersTranslationError::TargetOperation);
    };
    let TargetBooleanExpression::Parameter {
        source_value: right_value,
        parameter_index: right_parameter_index,
        location: right_location,
    } = right.as_ref()
    else {
        return Err(StraightLineBooleanEqualParametersTranslationError::TargetOperation);
    };
    if *psi_edge != reconstructed.return_edge
        || *source_value != reconstructed.source_value
        || *psi_operation != reconstructed.equal_operation
        || *left_value != reconstructed.left_value
        || *right_value != reconstructed.right_value
        || *left_parameter_index != reconstructed.left_parameter_index
        || *right_parameter_index != reconstructed.right_parameter_index
        || *left_location != reconstructed.left_location
        || *right_location != reconstructed.right_location
    {
        return Err(StraightLineBooleanEqualParametersTranslationError::TargetOperation);
    }
    Ok(StraightLineBooleanEqualParametersTranslationReceipt::new(
        source.machine,
        reconstructed.equal_operation,
        reconstructed.return_edge,
        reconstructed.source_value,
        reconstructed.left_value,
        reconstructed.right_value,
        reconstructed.left_parameter_index,
        reconstructed.right_parameter_index,
        reconstructed.left_location,
        reconstructed.right_location,
    ))
}
