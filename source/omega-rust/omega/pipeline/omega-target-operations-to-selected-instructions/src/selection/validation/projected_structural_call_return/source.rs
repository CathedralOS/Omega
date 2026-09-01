//! Independent source-shape and semantic-roster reconstruction.

use omega_calling_conventions::{ValueClass, ValuePlacement, ValueShape};
use omega_target_operations::TargetOperation;

use crate::selection::shared::*;

pub(super) fn replay(
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
    let Some(operation_result_placement) = callee_call_plan.result.clone() else {
        return Err(SelectedInstructionError::UnsupportedProjectedStructuralShape);
    };
    let Some(function_result_placement) = call_plan.result.clone() else {
        return Err(SelectedInstructionError::UnsupportedProjectedStructuralShape);
    };
    let [callee_parameter_placement] = callee_plan.parameters.as_slice() else {
        return Err(SelectedInstructionError::UnsupportedProjectedStructuralShape);
    };
    let placements = [
        (
            SelectedStructuralFragmentSite::CallerParameter,
            caller_parameter.placement.clone(),
        ),
        (
            SelectedStructuralFragmentSite::CallerArgumentSource,
            argument.source.clone(),
        ),
        (
            SelectedStructuralFragmentSite::CallerArgumentDestination,
            argument.destination.clone(),
        ),
        (
            SelectedStructuralFragmentSite::CallerOperationResult,
            operation_result_placement,
        ),
        (
            SelectedStructuralFragmentSite::CallerFunctionResult,
            function_result_placement,
        ),
        (
            SelectedStructuralFragmentSite::CalleeParameter,
            callee_parameter_placement.clone(),
        ),
        (
            SelectedStructuralFragmentSite::CalleeReturnSource,
            source_placement.clone(),
        ),
        (
            SelectedStructuralFragmentSite::CalleeFunctionResult,
            result_placement.clone(),
        ),
    ];
    let fragments = placements
        .into_iter()
        .map(|(site, placement)| fragment(site, placement))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((roster.clone(), fragments))
}

fn fragment(
    site: SelectedStructuralFragmentSite,
    placement: ValuePlacement,
) -> Result<SelectedStructuralFragmentConstraint, SelectedInstructionError> {
    let direct = matches!(
        placement.locations.as_slice(),
        [ValueLocation::Register {
            value_byte_offset: 0,
            byte_size: 8,
            ..
        }]
    );
    if placement.shape != ValueShape::integer(8, 8)
        || placement.shape.class != ValueClass::Integer
        || !direct
    {
        return Err(SelectedInstructionError::ProjectedStructuralConstraintMismatch { site });
    }
    Ok(SelectedStructuralFragmentConstraint { site, placement })
}
