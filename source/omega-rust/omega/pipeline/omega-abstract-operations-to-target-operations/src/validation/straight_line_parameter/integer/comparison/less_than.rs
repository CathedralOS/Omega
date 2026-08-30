//! Exact integer-parameter less-than target replay.

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::{
    TargetBooleanExpression, TargetFunction, TargetIntegerExpression, TargetOperation,
};

use super::super::super::super::{
    StraightLineIntegerLessThanParametersTranslationError,
    StraightLineIntegerLessThanParametersTranslationReceipt,
};
pub(crate) fn is_candidate(function: &AbstractFunction) -> bool {
    super::super::super::source::integer::comparison::less_than::is_candidate(function)
}

pub(crate) fn validate(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    StraightLineIntegerLessThanParametersTranslationReceipt,
    StraightLineIntegerLessThanParametersTranslationError,
> {
    let reconstructed = super::reconstruct_less_than(source, expected_target, target)?;
    let TargetOperation::ReturnBooleanExpression {
        psi_edge,
        source_value,
        expression:
            TargetBooleanExpression::IntegerLessThan {
                psi_operation,
                scalar_type,
                left,
                right,
            },
    } = &target.operation
    else {
        return Err(StraightLineIntegerLessThanParametersTranslationError::TargetOperation);
    };
    let TargetIntegerExpression::Parameter {
        source_value: left_value,
        parameter_index: left_parameter_index,
        location: left_location,
    } = left.as_ref()
    else {
        return Err(StraightLineIntegerLessThanParametersTranslationError::TargetOperation);
    };
    let TargetIntegerExpression::Parameter {
        source_value: right_value,
        parameter_index: right_parameter_index,
        location: right_location,
    } = right.as_ref()
    else {
        return Err(StraightLineIntegerLessThanParametersTranslationError::TargetOperation);
    };
    if *psi_edge != reconstructed.return_edge
        || *source_value != reconstructed.source_value
        || *psi_operation != reconstructed.operation
        || *scalar_type != reconstructed.scalar_type
        || *left_value != reconstructed.left_value
        || *right_value != reconstructed.right_value
        || *left_parameter_index != reconstructed.left_parameter_index
        || *right_parameter_index != reconstructed.right_parameter_index
        || *left_location != reconstructed.left_location
        || *right_location != reconstructed.right_location
    {
        return Err(StraightLineIntegerLessThanParametersTranslationError::TargetOperation);
    }
    Ok(
        StraightLineIntegerLessThanParametersTranslationReceipt::new(
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
