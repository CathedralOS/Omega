use omega_abstract_operations::{AbstractFunction, AbstractFunctionResult, AbstractResult};
use omega_target::NativeTarget;
use omega_target_operations::{TargetFunction, TargetOperation};
use psi_core::ScalarType;

use super::super::{
    StraightLineIntegerParameterTranslationError, StraightLineIntegerParameterTranslationReceipt,
};
use super::{model::ParameterResultKind, source};

pub(crate) fn is_candidate(function: &AbstractFunction) -> bool {
    source::direct::is_candidate(function, ParameterResultKind::Integer)
}

pub(crate) fn validate(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    StraightLineIntegerParameterTranslationReceipt,
    StraightLineIntegerParameterTranslationError,
> {
    let AbstractFunctionResult::Scalar(AbstractResult {
        scalar_type: ScalarType::Integer(result_type),
        ..
    }) = source.result
    else {
        return Err(StraightLineIntegerParameterTranslationError::SourceResult);
    };
    let reconstructed = super::reconstruct_parameter_return(
        source,
        expected_target,
        target,
        ScalarType::Integer(result_type),
    )?;
    if !matches!(
        target.operation,
        TargetOperation::ReturnIntegerParameter {
            psi_edge,
            source_value,
            scalar_type,
            parameter_index,
            location,
        } if psi_edge == reconstructed.return_edge
            && source_value == reconstructed.source_value
            && scalar_type == result_type
            && parameter_index == reconstructed.parameter_index
            && location == reconstructed.location
    ) {
        return Err(StraightLineIntegerParameterTranslationError::TargetOperation);
    }
    Ok(StraightLineIntegerParameterTranslationReceipt::new(
        source.machine,
        reconstructed.return_edge,
        reconstructed.source_value,
        result_type,
        reconstructed.parameter_index,
        reconstructed.location,
    ))
}
