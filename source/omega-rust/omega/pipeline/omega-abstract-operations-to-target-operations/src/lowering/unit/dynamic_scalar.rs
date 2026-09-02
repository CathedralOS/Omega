//! Exact target lowering for one rebound dynamic scalar call.

use super::super::scalar::scalar_shape;
use super::super::shared::*;
use super::super::structural_layout::structural_shape;
use super::projected_argument;
use super::scalar_call::{KnownUnitInteger, insert_known_unit_integer};

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
    let dispatch = &dynamic_dispatch.dispatch;
    if function.attachment.is_none()
        || !dynamic_dispatch.has_complete_application_custody(function.machine, *psi_operation)
    {
        return Err(LoweringError::InvalidDynamicScalarDispatch {
            machine: function.machine,
            operation: *psi_operation,
        });
    }
    let callee = dispatch.realization;
    let callee_function = functions
        .get(&callee)
        .copied()
        .ok_or(LoweringError::UnknownCallTarget(callee))?;
    let Some(callee_result) = callee_function.result.scalar() else {
        return Err(LoweringError::UnitCallTargetKindMismatch(callee));
    };
    let [callee_parameter] = callee_function.structural_parameters.as_slice() else {
        return Err(LoweringError::UnitCallTargetKindMismatch(callee));
    };
    if !callee_function.parameters.is_empty()
        || callee_result.scalar_type != result.scalar_type
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
    if !matches!(
        result.scalar_type,
        ScalarType::Boolean | ScalarType::Integer(_)
    ) {
        return Err(LoweringError::UnitScalarCallIntegerTypeUnsupported(
            result.value,
        ));
    }
    let result_shape = scalar_shape(result.value, result.scalar_type, false)?;
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
        call_plan,
        result_home,
        initial_argument,
        rebound_argument,
        requirement_obligations: requirement_obligations.clone(),
        crash_continuations: crash_continuations.clone(),
    });
    provenance.operations.push(*psi_operation);
    Ok(result_home)
}
