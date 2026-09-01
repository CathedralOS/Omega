//! Exact shift-right target replay with fixed carriers and obligation custody.

use omega_abstract_operations::AbstractFunction;
use omega_target::NativeTarget;
use omega_target_operations::{TargetFunction, TargetIntegerExpression, TargetOperation};

use super::super::super::super::{
    StraightLineExactIntegerShiftRightParametersTranslationError,
    StraightLineExactIntegerShiftRightParametersTranslationReceipt,
};

pub(crate) fn is_candidate(function: &AbstractFunction) -> bool {
    super::super::super::source::integer::shift::exact_right::is_candidate(function)
}

pub(crate) fn validate(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    StraightLineExactIntegerShiftRightParametersTranslationReceipt,
    StraightLineExactIntegerShiftRightParametersTranslationError,
> {
    let reconstructed = super::reconstruct_exact_right(source, expected_target, target)?;
    let shift = reconstructed.shift;
    let TargetOperation::ReturnIntegerExpression {
        psi_edge,
        source_value,
        scalar_type,
        expression:
            TargetIntegerExpression::ExactShiftRight {
                psi_operation,
                obligation,
                count_type,
                value,
                count,
            },
    } = &target.operation
    else {
        return Err(StraightLineExactIntegerShiftRightParametersTranslationError::TargetOperation);
    };
    let TargetIntegerExpression::Parameter {
        source_value: value_id,
        parameter_index: value_parameter_index,
        location: value_location,
    } = value.as_ref()
    else {
        return Err(StraightLineExactIntegerShiftRightParametersTranslationError::TargetOperation);
    };
    let TargetIntegerExpression::Parameter {
        source_value: count_id,
        parameter_index: count_parameter_index,
        location: count_location,
    } = count.as_ref()
    else {
        return Err(StraightLineExactIntegerShiftRightParametersTranslationError::TargetOperation);
    };
    if *psi_edge != shift.return_edge
        || *source_value != shift.source_value
        || *scalar_type != shift.value_type
        || *count_type != shift.count_type
        || *psi_operation != shift.operation
        || *obligation != reconstructed.obligation
        || *value_id != shift.value
        || *count_id != shift.count
        || *value_parameter_index != shift.value_parameter_index
        || *count_parameter_index != shift.count_parameter_index
        || *value_location != shift.value_location
        || *count_location != shift.count_location
    {
        return Err(StraightLineExactIntegerShiftRightParametersTranslationError::TargetOperation);
    }
    Ok(
        StraightLineExactIntegerShiftRightParametersTranslationReceipt::new(
            source.machine,
            shift.operation,
            reconstructed.obligation,
            shift.return_edge,
            shift.source_value,
            shift.value_type,
            shift.count_type,
            shift.value,
            shift.count,
            shift.value_parameter_index,
            shift.count_parameter_index,
            shift.value_location,
            shift.count_location,
        ),
    )
}
