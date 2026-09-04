//! Exact target lowering for one rebound dynamic call.

use super::super::scalar::scalar_shape;
use super::super::shared::*;
use super::super::structural_layout::structural_shape;
use super::projected_argument;
use super::scalar_call::{KnownUnitInteger, insert_known_unit_integer};
use omega_abstract_operations::{AbstractReboundDynamicDispatch, AbstractStoredDynamicDispatch};

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_stored_descriptor(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::StoreDynamicDescriptor {
        psi_operation,
        stored,
    } = operation
    else {
        unreachable!("stored descriptor lowering receives only descriptor stores")
    };
    let invalid = || LoweringError::InvalidDynamicDispatch {
        machine: function.machine,
        operation: *psi_operation,
    };
    if !stored.has_complete_custody(function.machine, *psi_operation) {
        return Err(invalid());
    }
    let store_index = function
        .operations
        .iter()
        .position(|candidate| {
            matches!(candidate,
                AbstractOperation::StoreDynamicDescriptor {
                    psi_operation: candidate_operation,
                    ..
                } if candidate_operation == psi_operation)
        })
        .ok_or_else(invalid)?;
    let calls = function
        .operations
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| match candidate {
            AbstractOperation::CallStoredDynamicScalar {
                psi_operation,
                dynamic_dispatch,
                result,
                ..
            } if &dynamic_dispatch.stored == stored && index > store_index => {
                Some((*psi_operation, dynamic_dispatch, result))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [(call_operation, dispatch, result)] = calls.as_slice() else {
        return Err(invalid());
    };
    let result_shape = scalar_shape(result.value, result.scalar_type, false)?;
    let lowered = lower_stored_call(
        function,
        target,
        functions,
        structural_types,
        parameters_by_place,
        shape_cache,
        active,
        *call_operation,
        dispatch,
        result.scalar_type,
        result_shape,
    )?;
    operations.push(TargetUnitOperation::StoreDynamicDescriptor {
        psi_operation: *psi_operation,
        stored: stored.clone(),
        source_argument: lowered.source_argument,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_stored_dynamic_scalar_call(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    scalar_values: &mut BTreeMap<ValueId, KnownUnitInteger>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<TargetUnitScalarHomeRequirement, LoweringError> {
    let AbstractOperation::CallStoredDynamicScalar {
        psi_operation,
        result,
        dynamic_dispatch,
        requirement_obligations,
        crash_continuations,
    } = operation
    else {
        unreachable!("stored dynamic scalar lowering receives only stored calls")
    };
    if !matches!(
        result.scalar_type,
        ScalarType::Boolean | ScalarType::Integer(_)
    ) {
        return Err(LoweringError::UnitScalarCallIntegerTypeUnsupported(
            result.value,
        ));
    }
    let call_index = function
        .operations
        .iter()
        .position(|candidate| {
            matches!(candidate,
                AbstractOperation::CallStoredDynamicScalar {
                    psi_operation: candidate_operation,
                    ..
                } if candidate_operation == psi_operation)
        })
        .ok_or(LoweringError::InvalidDynamicDispatch {
            machine: function.machine,
            operation: *psi_operation,
        })?;
    let store_count = function.operations[..call_index]
        .iter()
        .filter(|candidate| {
            matches!(candidate,
                AbstractOperation::StoreDynamicDescriptor { stored, .. }
                    if stored == &dynamic_dispatch.stored)
        })
        .count();
    if store_count != 1 {
        return Err(LoweringError::InvalidDynamicDispatch {
            machine: function.machine,
            operation: *psi_operation,
        });
    }
    let result_shape = scalar_shape(result.value, result.scalar_type, false)?;
    let lowered = lower_stored_call(
        function,
        target,
        functions,
        structural_types,
        parameters_by_place,
        shape_cache,
        active,
        *psi_operation,
        dynamic_dispatch,
        result.scalar_type,
        result_shape,
    )?;
    let result_home = TargetUnitScalarHomeRequirement {
        defining_operation: *psi_operation,
        source_value: result.value,
        scalar_type: result.scalar_type,
        shape: result_shape,
    };
    if matches!(result.scalar_type, ScalarType::Integer(_)) {
        insert_known_unit_integer(
            scalar_values,
            result.value,
            KnownUnitInteger::Home(result_home),
        )?;
    }
    operations.push(TargetUnitOperation::StoredDynamicScalarCall {
        psi_operation: *psi_operation,
        result: *result,
        dynamic_dispatch: dynamic_dispatch.clone(),
        call_plan: lowered.call_plan,
        result_home,
        source_argument: lowered.source_argument,
        requirement_obligations: requirement_obligations.clone(),
        crash_continuations: crash_continuations.clone(),
    });
    provenance.operations.push(*psi_operation);
    Ok(result_home)
}

struct LoweredStoredDynamicCall {
    call_plan: CallPlan,
    source_argument: TargetStructuralArgument,
}

#[allow(clippy::too_many_arguments)]
fn lower_stored_call(
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    psi_operation: OperationId,
    dynamic_dispatch: &AbstractStoredDynamicDispatch,
    expected_result: ScalarType,
    result_shape: ValueShape,
) -> Result<LoweredStoredDynamicCall, LoweringError> {
    let invalid = || LoweringError::InvalidDynamicDispatch {
        machine: function.machine,
        operation: psi_operation,
    };
    if function.attachment.is_none()
        || !dynamic_dispatch.has_complete_custody(function.machine, psi_operation)
    {
        return Err(invalid());
    }
    let callee = dynamic_dispatch.dispatch.realization;
    let callee_function = functions
        .get(&callee)
        .copied()
        .ok_or(LoweringError::UnknownCallTarget(callee))?;
    let [callee_parameter] = callee_function.structural_parameters.as_slice() else {
        return Err(LoweringError::UnitCallTargetKindMismatch(callee));
    };
    if !callee_function.parameters.is_empty()
        || !matches!(
            callee_function.result,
            AbstractFunctionResult::Scalar(result) if result.scalar_type == expected_result
        )
        || !callee_function.published_service_ceiling.is_empty()
    {
        return Err(LoweringError::UnitCallTargetKindMismatch(callee));
    }
    let argument_shape = structural_shape(
        callee_parameter.structural_type,
        structural_types,
        shape_cache,
        active,
    )?;
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![argument_shape],
            result: Some(result_shape),
        },
    )
    .map_err(LoweringError::AbiPlan)?;
    let [destination] = call_plan.parameters.as_slice() else {
        return Err(LoweringError::UnitCallTargetKindMismatch(callee));
    };
    if call_plan.result.as_ref().map(|placement| placement.shape) != Some(result_shape) {
        return Err(LoweringError::UnitCallTargetKindMismatch(callee));
    }
    let source_argument = projected_argument::lower(
        function.machine,
        callee,
        &dynamic_dispatch.stored.selection.source,
        callee_parameter,
        destination,
        structural_types,
        parameters_by_place,
        shape_cache,
        active,
    )?;
    Ok(LoweredStoredDynamicCall {
        call_plan,
        source_argument,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_dynamic_scalar_call(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    scalar_values: &mut BTreeMap<ValueId, KnownUnitInteger>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<TargetUnitScalarHomeRequirement, LoweringError> {
    let AbstractOperation::CallDynamicScalar {
        psi_operation,
        result,
        dynamic_dispatch,
        requirement_obligations,
        crash_continuations,
    } = operation
    else {
        unreachable!("dynamic scalar lowering receives only dynamic calls")
    };
    if !matches!(
        result.scalar_type,
        ScalarType::Boolean | ScalarType::Integer(_)
    ) {
        return Err(LoweringError::UnitScalarCallIntegerTypeUnsupported(
            result.value,
        ));
    }
    let result_shape = scalar_shape(result.value, result.scalar_type, false)?;
    let lowered = lower_dynamic_call(
        function,
        target,
        functions,
        structural_types,
        parameters_by_place,
        shape_cache,
        active,
        *psi_operation,
        dynamic_dispatch,
        Some(result.scalar_type),
        Some(result_shape),
    )?;
    let result_home = TargetUnitScalarHomeRequirement {
        defining_operation: *psi_operation,
        source_value: result.value,
        scalar_type: result.scalar_type,
        shape: result_shape,
    };
    if matches!(result.scalar_type, ScalarType::Integer(_)) {
        insert_known_unit_integer(
            scalar_values,
            result.value,
            KnownUnitInteger::Home(result_home),
        )?;
    }
    operations.push(TargetUnitOperation::DynamicScalarCall {
        psi_operation: *psi_operation,
        result: *result,
        dynamic_dispatch: dynamic_dispatch.clone(),
        call_plan: lowered.call_plan,
        result_home,
        initial_argument: lowered.initial_argument,
        rebound_argument: lowered.rebound_argument,
        requirement_obligations: requirement_obligations.clone(),
        crash_continuations: crash_continuations.clone(),
    });
    provenance.operations.push(*psi_operation);
    Ok(result_home)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_dynamic_unit_call(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::CallDynamicUnit {
        psi_operation,
        dynamic_dispatch,
        requirement_obligations,
        crash_continuations,
    } = operation
    else {
        unreachable!("dynamic Unit lowering receives only dynamic Unit calls")
    };
    let lowered = lower_dynamic_call(
        function,
        target,
        functions,
        structural_types,
        parameters_by_place,
        shape_cache,
        active,
        *psi_operation,
        dynamic_dispatch,
        None,
        None,
    )?;
    operations.push(TargetUnitOperation::DynamicUnitCall {
        psi_operation: *psi_operation,
        dynamic_dispatch: dynamic_dispatch.clone(),
        call_plan: lowered.call_plan,
        initial_argument: lowered.initial_argument,
        rebound_argument: lowered.rebound_argument,
        requirement_obligations: requirement_obligations.clone(),
        crash_continuations: crash_continuations.clone(),
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

struct LoweredDynamicCall {
    call_plan: CallPlan,
    initial_argument: TargetStructuralArgument,
    rebound_argument: TargetStructuralArgument,
}

#[allow(clippy::too_many_arguments)]
fn lower_dynamic_call(
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    psi_operation: OperationId,
    dynamic_dispatch: &AbstractReboundDynamicDispatch,
    expected_result: Option<ScalarType>,
    result_shape: Option<ValueShape>,
) -> Result<LoweredDynamicCall, LoweringError> {
    let invalid = || LoweringError::InvalidDynamicDispatch {
        machine: function.machine,
        operation: psi_operation,
    };
    if function.attachment.is_none()
        || !dynamic_dispatch.has_complete_application_custody(function.machine, psi_operation)
    {
        return Err(invalid());
    }
    let callee = dynamic_dispatch.dispatch.realization;
    let callee_function = functions
        .get(&callee)
        .copied()
        .ok_or(LoweringError::UnknownCallTarget(callee))?;
    let result_matches = match (&callee_function.result, expected_result) {
        (AbstractFunctionResult::Unit, None) => true,
        (AbstractFunctionResult::Scalar(result), Some(expected)) => result.scalar_type == expected,
        _ => false,
    };
    let [callee_parameter] = callee_function.structural_parameters.as_slice() else {
        return Err(LoweringError::UnitCallTargetKindMismatch(callee));
    };
    if !callee_function.parameters.is_empty()
        || !result_matches
        || !callee_function.published_service_ceiling.is_empty()
    {
        return Err(LoweringError::UnitCallTargetKindMismatch(callee));
    }
    let argument_shape = structural_shape(
        callee_parameter.structural_type,
        structural_types,
        shape_cache,
        active,
    )?;
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![argument_shape],
            result: result_shape,
        },
    )
    .map_err(LoweringError::AbiPlan)?;
    let [destination] = call_plan.parameters.as_slice() else {
        return Err(LoweringError::UnitCallTargetKindMismatch(callee));
    };
    if call_plan.result.as_ref().map(|placement| placement.shape) != result_shape {
        return Err(LoweringError::UnitCallTargetKindMismatch(callee));
    }
    let initial_argument = projected_argument::lower(
        function.machine,
        callee,
        &dynamic_dispatch.initial.source,
        callee_parameter,
        destination,
        structural_types,
        parameters_by_place,
        shape_cache,
        active,
    )?;
    let rebound_argument = projected_argument::lower(
        function.machine,
        callee,
        &dynamic_dispatch.rebound.source,
        callee_parameter,
        destination,
        structural_types,
        parameters_by_place,
        shape_cache,
        active,
    )?;
    Ok(LoweredDynamicCall {
        call_plan,
        initial_argument,
        rebound_argument,
    })
}
