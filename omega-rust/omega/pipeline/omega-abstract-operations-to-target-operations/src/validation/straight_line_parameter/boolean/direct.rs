use omega_abstract_operations::{AbstractFunction, AbstractFunctionResult, AbstractResult};
use omega_target::NativeTarget;
use omega_target_operations::{TargetFunction, TargetOperation};
use psi_core::ScalarType;

use super::super::super::{
    StraightLineBooleanParameterTranslationError, StraightLineBooleanParameterTranslationReceipt,
};
use super::super::{model::ParameterResultKind, source};

pub(crate) fn is_candidate(function: &AbstractFunction) -> bool {
    source::direct::is_candidate(function, ParameterResultKind::Boolean)
}

pub(crate) fn validate(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    StraightLineBooleanParameterTranslationReceipt,
    StraightLineBooleanParameterTranslationError,
> {
    if !matches!(
        source.result,
        AbstractFunctionResult::Scalar(AbstractResult {
            scalar_type: ScalarType::Boolean,
            ..
        })
    ) {
        return Err(StraightLineBooleanParameterTranslationError::SourceResult);
    }
    let reconstructed = super::super::reconstruct_parameter_return(
        source,
        expected_target,
        target,
        ScalarType::Boolean,
    )?;
    if !matches!(
        target.operation,
        TargetOperation::ReturnBooleanParameter {
            psi_edge,
            source_value,
            parameter_index,
            location,
        } if psi_edge == reconstructed.return_edge
            && source_value == reconstructed.source_value
            && parameter_index == reconstructed.parameter_index
            && location == reconstructed.location
    ) {
        return Err(StraightLineBooleanParameterTranslationError::TargetOperation);
    }
    Ok(StraightLineBooleanParameterTranslationReceipt::new(
        source.machine,
        reconstructed.return_edge,
        reconstructed.source_value,
        reconstructed.parameter_index,
        reconstructed.location,
    ))
}
