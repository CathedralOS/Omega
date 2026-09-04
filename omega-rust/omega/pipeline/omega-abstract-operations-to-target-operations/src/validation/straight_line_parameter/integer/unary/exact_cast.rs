//! Typed target replay for an exact proof-bearing integer cast of one parameter.

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::{TargetFunction, TargetIntegerExpression, TargetOperation};

use super::super::super::super::{
    StraightLineIntegerExactCastParameterTranslationError,
    StraightLineIntegerExactCastParameterTranslationReceipt,
};

pub(crate) fn is_candidate(function: &AbstractFunction) -> bool {
    super::super::super::source::integer::unary::exact_cast::is_candidate(function)
}

pub(crate) fn validate(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    StraightLineIntegerExactCastParameterTranslationReceipt,
    StraightLineIntegerExactCastParameterTranslationError,
> {
    let reconstructed = super::replay::reconstruct_exact_cast(source, expected_target, target)?;
    let TargetOperation::ReturnIntegerExpression {
        psi_edge,
        source_value,
        scalar_type,
        expression:
            TargetIntegerExpression::IntegerExactCast {
                psi_operation,
                obligation,
                source_type,
                operand,
            },
    } = &target.operation
    else {
        return Err(StraightLineIntegerExactCastParameterTranslationError::TargetOperation);
    };
    let TargetIntegerExpression::Parameter {
        source_value: operand_value,
        parameter_index,
        location,
    } = operand.as_ref()
    else {
        return Err(StraightLineIntegerExactCastParameterTranslationError::TargetOperation);
    };
    if *psi_edge != reconstructed.return_edge
        || *source_value != reconstructed.source_value
        || *scalar_type != reconstructed.target_type
        || *psi_operation != reconstructed.operation
        || *obligation != reconstructed.obligation
        || *source_type != reconstructed.source_type
        || *operand_value != reconstructed.operand_value
        || *parameter_index != reconstructed.parameter_index
        || *location != reconstructed.location
    {
        return Err(StraightLineIntegerExactCastParameterTranslationError::TargetOperation);
    }
    Ok(
        StraightLineIntegerExactCastParameterTranslationReceipt::new(
            source.machine,
            reconstructed.operation,
            reconstructed.obligation,
            reconstructed.return_edge,
            reconstructed.source_value,
            reconstructed.source_type,
            reconstructed.target_type,
            reconstructed.operand_value,
            reconstructed.parameter_index,
            reconstructed.location,
        ),
    )
}
