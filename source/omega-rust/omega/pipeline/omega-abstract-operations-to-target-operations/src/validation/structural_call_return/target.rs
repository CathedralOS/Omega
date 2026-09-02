//! Independent target-carrier replay. This module does not import producer mechanics.

use omega_calling_conventions::{CallSignature, CallingPolicy, evaluate_call_plan};
use omega_target_operations::{TargetOperation, TargetOperationPlan, TargetStructuralParameter};

use super::model::{
    StructuralCallReturnProjectedQualificationValidationError as Error,
    StructuralCallReturnRosterLocation as Location, StructuralCallReturnSource, is_canonical,
};

pub(super) fn replay(
    source: &StructuralCallReturnSource,
    target: &TargetOperationPlan,
) -> Result<(), Error> {
    if target.functions.len() != 2 {
        return Err(Error::TargetShape);
    }
    let caller = target
        .functions
        .iter()
        .find(|function| function.machine == source.caller)
        .ok_or(Error::TargetMachineMismatch)?;
    let callee = target
        .functions
        .iter()
        .find(|function| function.machine == source.callee)
        .ok_or(Error::TargetMachineMismatch)?;
    let TargetOperation::ReturnStructuralCall {
        operation_result,
        result,
        callee: target_callee,
        structural_types,
        call_plan,
        callee_call_plan,
        structural_parameters,
        arguments,
        ..
    } = &caller.operation
    else {
        return Err(Error::TargetShape);
    };
    let [target_parameter] = structural_parameters.as_slice() else {
        return Err(Error::TargetShape);
    };
    let [argument] = arguments.as_slice() else {
        return Err(Error::TargetShape);
    };
    if *target_callee != source.callee {
        return Err(Error::TargetCalleeMismatch);
    }
    replay_target_parameter(source, target_parameter)?;
    let expected_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target.target),
        &CallSignature {
            parameters: vec![source.shape],
            result: Some(source.shape),
        },
    )
    .map_err(|_| Error::TargetShape)?;
    let Some(expected_parameter_placement) = expected_plan.parameters.first() else {
        return Err(Error::TargetShape);
    };
    if structural_types != &source.structural_types
        || call_plan != &expected_plan
        || callee_call_plan != &expected_plan
        || target_parameter.shape != source.shape
        || target_parameter.placement != *expected_parameter_placement
        || argument.place != source.caller_parameter.place
        || argument.access != source.caller_parameter.access
        || !argument.path.is_empty()
        || argument.root_structural_type != source.caller_parameter.structural_type
        || argument.structural_type != source.caller_parameter.structural_type
        || argument.shape != source.shape
        || argument.source_byte_offset != 0
        || argument.fixed_array_length.is_some()
        || argument.element_stride.is_some()
        || argument.source != *expected_parameter_placement
        || argument.destination != *expected_parameter_placement
    {
        return Err(Error::TargetShape);
    }
    require_target_roster(
        &source.caller_operation_result.projected_qualifications,
        &operation_result.projected_qualifications,
        Location::CallerOperationResult,
    )?;
    if operation_result != &source.caller_operation_result {
        return Err(Error::TargetRosterMismatch(Location::CallerOperationResult));
    }
    require_target_roster(
        &source.caller_result.projected_qualifications,
        &result.projected_qualifications,
        Location::CallerFunctionResult,
    )?;
    if result != &source.caller_result {
        return Err(Error::TargetRosterMismatch(Location::CallerFunctionResult));
    }

    let TargetOperation::ReturnStructuralParameter {
        call_plan,
        scalar_parameters,
        parameters,
        source: target_source,
        result: target_result,
        shape,
        source_placement,
        result_placement,
        ..
    } = &callee.operation
    else {
        return Err(Error::TargetShape);
    };
    let [target_callee_parameter] = parameters.as_slice() else {
        return Err(Error::TargetShape);
    };
    if !scalar_parameters.is_empty()
        || call_plan != &expected_plan
        || *shape != source.shape
        || source_placement != expected_parameter_placement
        || expected_plan.result.as_ref() != Some(result_placement)
    {
        return Err(Error::TargetShape);
    }
    require_target_roster(
        &source.callee_parameter.projected_qualifications,
        &target_callee_parameter.projected_qualifications,
        Location::CalleeParameter,
    )?;
    if target_callee_parameter != &source.callee_parameter {
        return Err(Error::TargetRosterMismatch(Location::CalleeParameter));
    }
    require_target_roster(
        &source.callee_parameter.projected_qualifications,
        &target_source.projected_qualifications,
        Location::CalleeSource,
    )?;
    if target_source != &source.callee_parameter {
        return Err(Error::TargetRosterMismatch(Location::CalleeSource));
    }
    require_target_roster(
        &source.callee_result.projected_qualifications,
        &target_result.projected_qualifications,
        Location::CalleeFunctionResult,
    )?;
    if target_result != &source.callee_result {
        return Err(Error::TargetRosterMismatch(Location::CalleeFunctionResult));
    }
    Ok(())
}

fn replay_target_parameter(
    source: &StructuralCallReturnSource,
    target: &TargetStructuralParameter,
) -> Result<(), Error> {
    require_target_roster(
        &source.roster,
        &target.projected_qualifications,
        Location::TargetParameter,
    )?;
    let expected = &source.caller_parameter;
    if target.place != expected.place
        || target.structural_type != expected.structural_type
        || target.multiplicity != expected.multiplicity
        || target.access != expected.access
    {
        return Err(Error::TargetRosterMismatch(Location::TargetParameter));
    }
    Ok(())
}

fn require_target_roster(
    expected: &[psi_terminal::StructuralPathQualification],
    actual: &[psi_terminal::StructuralPathQualification],
    location: Location,
) -> Result<(), Error> {
    if !is_canonical(actual) {
        return Err(Error::TargetRosterNotCanonical(location));
    }
    if actual != expected {
        return Err(Error::TargetRosterMismatch(location));
    }
    Ok(())
}
