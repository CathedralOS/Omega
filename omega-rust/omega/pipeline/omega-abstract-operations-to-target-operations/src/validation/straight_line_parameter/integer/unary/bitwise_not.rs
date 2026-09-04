//! Typed target replay for integer bitwise-not of one parameter.

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::{TargetFunction, TargetIntegerExpression, TargetOperation};

use super::super::super::super::{
    StraightLineIntegerBitwiseNotParameterTranslationError,
    StraightLineIntegerBitwiseNotParameterTranslationReceipt,
};

pub(crate) fn is_candidate(function: &AbstractFunction) -> bool {
    super::super::super::source::integer::unary::bitwise_not::is_candidate(function)
}

pub(crate) fn validate(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    StraightLineIntegerBitwiseNotParameterTranslationReceipt,
    StraightLineIntegerBitwiseNotParameterTranslationError,
> {
    let reconstructed = super::replay::reconstruct_bitwise_not(source, expected_target, target)?;
    let TargetOperation::ReturnIntegerExpression {
        psi_edge,
        source_value,
        scalar_type,
        expression:
            TargetIntegerExpression::BitwiseNot {
                psi_operation,
                operand,
            },
    } = &target.operation
    else {
        return Err(StraightLineIntegerBitwiseNotParameterTranslationError::TargetOperation);
    };
    let TargetIntegerExpression::Parameter {
        source_value: operand_value,
        parameter_index,
        location,
    } = operand.as_ref()
    else {
        return Err(StraightLineIntegerBitwiseNotParameterTranslationError::TargetOperation);
    };
    if *psi_edge != reconstructed.return_edge
        || *source_value != reconstructed.source_value
        || *scalar_type != reconstructed.scalar_type
        || *psi_operation != reconstructed.operation
        || *operand_value != reconstructed.operand_value
        || *parameter_index != reconstructed.parameter_index
        || *location != reconstructed.location
    {
        return Err(StraightLineIntegerBitwiseNotParameterTranslationError::TargetOperation);
    }
    Ok(
        StraightLineIntegerBitwiseNotParameterTranslationReceipt::new(
            source.machine,
            reconstructed.operation,
            reconstructed.return_edge,
            reconstructed.source_value,
            reconstructed.scalar_type,
            reconstructed.operand_value,
            reconstructed.parameter_index,
            reconstructed.location,
        ),
    )
}
