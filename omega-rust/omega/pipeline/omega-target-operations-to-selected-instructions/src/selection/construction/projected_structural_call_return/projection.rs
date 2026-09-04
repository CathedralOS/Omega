use omega_calling_conventions::{ValueClass, ValuePlacement, ValueShape};
use omega_target_operations::TargetOperation;

use crate::selection::shared::*;

pub(super) fn project(
    source: &LegalizedProjectedStructuralCallReturn,
) -> Result<
    (
        Vec<psi_terminal::StructuralPathQualification>,
        Vec<SelectedStructuralFragmentConstraint>,
    ),
    SelectedInstructionError,
> {
    let TargetOperation::ReturnStructuralCall {
        operation_result,
        result,
        call_plan,
        callee_call_plan,
        structural_parameters,
        arguments,
        ..
    } = &source.caller.operation
    else {
        return Err(SelectedInstructionError::UnsupportedProjectedStructuralShape);
    };
    let TargetOperation::ReturnStructuralParameter {
        call_plan: callee_plan,
        scalar_parameters,
        parameters,
        source: callee_source,
        result: callee_result,
        source_placement,
        result_placement,
        ..
    } = &source.callee.operation
    else {
        return Err(SelectedInstructionError::UnsupportedProjectedStructuralShape);
    };
    if !scalar_parameters.is_empty() {
        return Err(SelectedInstructionError::UnsupportedProjectedStructuralShape);
    }
    let ([caller_parameter], [argument], [callee_parameter]) = (
        structural_parameters.as_slice(),
        arguments.as_slice(),
        parameters.as_slice(),
    ) else {
        return Err(SelectedInstructionError::UnsupportedProjectedStructuralShape);
    };
    let roster = &caller_parameter.projected_qualifications;
    if roster.is_empty()
        || roster != &operation_result.projected_qualifications
        || roster != &result.projected_qualifications
        || roster != &callee_parameter.projected_qualifications
        || roster != &callee_source.projected_qualifications
        || roster != &callee_result.projected_qualifications
    {
        return Err(SelectedInstructionError::ProjectedStructuralRosterMismatch);
    }
    let Some(caller_operation_result) = callee_call_plan.result.clone() else {
        return Err(SelectedInstructionError::UnsupportedProjectedStructuralShape);
    };
    let Some(caller_function_result) = call_plan.result.clone() else {
        return Err(SelectedInstructionError::UnsupportedProjectedStructuralShape);
    };
    let [callee_parameter_placement] = callee_plan.parameters.as_slice() else {
        return Err(SelectedInstructionError::UnsupportedProjectedStructuralShape);
    };
    let fragments = vec![
        constraint(
            SelectedStructuralFragmentSite::CallerParameter,
            caller_parameter.placement.clone(),
        )?,
        constraint(
            SelectedStructuralFragmentSite::CallerArgumentSource,
            argument.source.clone(),
        )?,
        constraint(
            SelectedStructuralFragmentSite::CallerArgumentDestination,
            argument.destination.clone(),
        )?,
        constraint(
            SelectedStructuralFragmentSite::CallerOperationResult,
            caller_operation_result,
        )?,
        constraint(
            SelectedStructuralFragmentSite::CallerFunctionResult,
            caller_function_result,
        )?,
        constraint(
            SelectedStructuralFragmentSite::CalleeParameter,
            callee_parameter_placement.clone(),
        )?,
        constraint(
            SelectedStructuralFragmentSite::CalleeReturnSource,
            source_placement.clone(),
        )?,
        constraint(
            SelectedStructuralFragmentSite::CalleeFunctionResult,
            result_placement.clone(),
        )?,
    ];
    Ok((roster.clone(), fragments))
}

fn constraint(
    site: SelectedStructuralFragmentSite,
    placement: ValuePlacement,
) -> Result<SelectedStructuralFragmentConstraint, SelectedInstructionError> {
    if !is_exact_fragment(&placement) {
        return Err(SelectedInstructionError::ProjectedStructuralConstraintMismatch { site });
    }
    Ok(SelectedStructuralFragmentConstraint { site, placement })
}

fn is_exact_fragment(placement: &ValuePlacement) -> bool {
    placement.shape == ValueShape::integer(8, 8)
        && placement.shape.class == ValueClass::Integer
        && matches!(placement.locations.as_slice(), [location] if match location {
            omega_calling_conventions::ValueLocation::Register {
                value_byte_offset,
                byte_size,
                ..
            } => *value_byte_offset == 0 && *byte_size == 8,
            omega_calling_conventions::ValueLocation::Stack { .. } => false,
            omega_calling_conventions::ValueLocation::Indirect { .. } => false,
        })
}
