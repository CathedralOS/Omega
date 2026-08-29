use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::{TargetFunction, TargetOperation};

use super::super::{
    StraightLineBooleanNotParameterTranslationError,
    StraightLineBooleanNotParameterTranslationReceipt,
};
use super::source;

pub(crate) fn is_candidate(function: &AbstractFunction) -> bool {
    source::boolean_not::is_candidate(function)
}

pub(crate) fn validate(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    StraightLineBooleanNotParameterTranslationReceipt,
    StraightLineBooleanNotParameterTranslationError,
> {
    let reconstructed = super::reconstruct_boolean_not_parameter(source, expected_target, target)?;
    if !matches!(
        target.operation,
        TargetOperation::ReturnBooleanNotParameter {
            psi_edge,
            source_value,
            parameter_index,
            location,
        } if psi_edge == reconstructed.return_edge
            && source_value == reconstructed.source_value
            && parameter_index == reconstructed.parameter_index
            && location == reconstructed.location
    ) {
        return Err(StraightLineBooleanNotParameterTranslationError::TargetOperation);
    }
    Ok(StraightLineBooleanNotParameterTranslationReceipt::new(
        source.machine,
        reconstructed.not_operation,
        reconstructed.return_edge,
        reconstructed.source_value,
        reconstructed.operand_value,
        reconstructed.parameter_index,
        reconstructed.location,
    ))
}
