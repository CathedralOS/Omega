//! Exact integer-remainder replay with defined-division obligation custody.

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::{TargetFunction, TargetIntegerExpression, TargetOperation};

use super::super::super::super::{
    StraightLineExactIntegerRemainderParametersTranslationError,
    StraightLineExactIntegerRemainderParametersTranslationReceipt,
};

pub(crate) fn is_candidate(function: &AbstractFunction) -> bool {
    super::super::super::source::integer::arithmetic::exact_remainder::is_candidate(function)
}

pub(crate) fn validate(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    StraightLineExactIntegerRemainderParametersTranslationReceipt,
    StraightLineExactIntegerRemainderParametersTranslationError,
> {
    let reconstructed = super::reconstruct_exact_remainder(source, expected_target, target)?;
    let arithmetic = reconstructed.arithmetic;
    let TargetOperation::ReturnIntegerExpression {
        psi_edge,
        source_value,
        scalar_type,
        expression:
            TargetIntegerExpression::ExactRemainder {
                psi_operation,
                obligation,
                left,
                right,
            },
    } = &target.operation
    else {
        return Err(StraightLineExactIntegerRemainderParametersTranslationError::TargetOperation);
    };
    let TargetIntegerExpression::Parameter {
        source_value: left_value,
        parameter_index: left_parameter_index,
        location: left_location,
    } = left.as_ref()
    else {
        return Err(StraightLineExactIntegerRemainderParametersTranslationError::TargetOperation);
    };
    let TargetIntegerExpression::Parameter {
        source_value: right_value,
        parameter_index: right_parameter_index,
        location: right_location,
    } = right.as_ref()
    else {
        return Err(StraightLineExactIntegerRemainderParametersTranslationError::TargetOperation);
    };
    if *psi_edge != arithmetic.return_edge
        || *source_value != arithmetic.source_value
        || *scalar_type != arithmetic.scalar_type
        || *psi_operation != arithmetic.operation
        || *obligation != reconstructed.obligation
        || *left_value != arithmetic.left_value
        || *right_value != arithmetic.right_value
        || *left_parameter_index != arithmetic.left_parameter_index
        || *right_parameter_index != arithmetic.right_parameter_index
        || *left_location != arithmetic.left_location
        || *right_location != arithmetic.right_location
    {
        return Err(StraightLineExactIntegerRemainderParametersTranslationError::TargetOperation);
    }
    Ok(
        StraightLineExactIntegerRemainderParametersTranslationReceipt::new(
            source.machine,
            arithmetic.operation,
            reconstructed.obligation,
            arithmetic.return_edge,
            arithmetic.source_value,
            arithmetic.scalar_type,
            arithmetic.left_value,
            arithmetic.right_value,
            arithmetic.left_parameter_index,
            arithmetic.right_parameter_index,
            arithmetic.left_location,
            arithmetic.right_location,
        ),
    )
}
