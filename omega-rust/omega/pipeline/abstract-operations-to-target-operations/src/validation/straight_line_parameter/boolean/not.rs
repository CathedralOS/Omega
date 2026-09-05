use abstract_operations::AbstractFunction;
use semantic_vocabulary::ScalarType;
use target::NativeTarget;
use target_operations::{TargetFunction, TargetOperation};

use super::super::super::{
    StraightLineBooleanNotParameterTranslationError,
    StraightLineBooleanNotParameterTranslationReceipt,
};
use super::super::{abi, model::ReconstructedBooleanNotParameter, source};

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
    let reconstructed = reconstruct(source, expected_target, target)?;
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

fn reconstruct(
    function: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<ReconstructedBooleanNotParameter, StraightLineBooleanNotParameterTranslationError> {
    let source = source::reconstruct_boolean_not(function)?;
    let locations = abi::replay(&function.parameters, ScalarType::Boolean, expected_target)?;
    if target.provenance.operations.as_slice() != [source.not_operation]
        || target.provenance.edges.as_slice() != [source.return_edge]
    {
        return Err(StraightLineBooleanNotParameterTranslationError::TargetProvenance);
    }
    Ok(ReconstructedBooleanNotParameter {
        not_operation: source.not_operation,
        return_edge: source.return_edge,
        source_value: source.source_value,
        operand_value: source.operand_value,
        parameter_index: source.parameter_index,
        location: locations[source.parameter_index],
    })
}
