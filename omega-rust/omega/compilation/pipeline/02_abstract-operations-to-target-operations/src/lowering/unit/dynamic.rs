//! Exact target lowering for every call whose ABI transports existential
//! descriptors, and for the descriptor stores such calls reload.
//!
//! `lower_dynamic_call` is the one entry. Each dispatch kind derives its own
//! call plan and custody row: a direct call forwarding descriptor pairs
//! (`descriptor_arguments`), a stored or rebound descriptor (here), or the
//! function's own descriptor parameter (`parameter`). Every kind admits its
//! result role through the shared `dynamic_result_home`, and one
//! `TargetUnitOperation::CallDynamic` row retains the dispatch, plan and
//! result together.

mod descriptor_arguments;
mod parameter;

use super::super::scalar::scalar_shape;
use super::super::structural_signature::StructuralCallSignature;
use super::known_integers::{KnownUnitInteger, insert_known_unit_integer};
use super::projected_argument;
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{
    AbstractDynamicCallResult, AbstractDynamicDispatch, AbstractFunction, AbstractFunctionResult,
    AbstractOperation, AbstractResult, AbstractStructuralCallResult,
};
use abstract_operations::{AbstractReboundDynamicDispatch, AbstractStoredDynamicDispatch};
use calling_conventions::{CallPlan, ValueShape};
use semantic_vocabulary::{MachineId, OperationId, PlaceId, ScalarType, StructuralTypeId, ValueId};
use std::collections::{BTreeMap, BTreeSet};
use target::NativeTarget;
use target_operations::{
    TargetDynamicCallResult, TargetDynamicDescriptorParameterAbi, TargetDynamicDispatch,
    TargetStructuralArgument, TargetStructuralParameter, TargetUnitOperation,
    TargetUnitScalarHomeRequirement, TerminalPsiProvenance,
};

/// Whether `operation` lowers to one `CallDynamic` row: every descriptor
/// dispatch, and a direct Unit- or scalar-result call that forwards
/// descriptors. A structural result never forwards descriptors; the ordinary
/// structural call lowering refuses one that does.
pub(in crate::lowering) fn is_dynamic_call(operation: &AbstractOperation) -> bool {
    match operation {
        AbstractOperation::CallDynamic { .. } => true,
        AbstractOperation::CallStructural {
            dynamic_arguments,
            result: AbstractStructuralCallResult::Unit | AbstractStructuralCallResult::Scalar(_),
            ..
        } => !dynamic_arguments.is_empty(),
        _ => false,
    }
}

/// One dispatch kind's lowered custody, the evaluated plan of the native call,
/// and the scalar home its result role defines.
struct LoweredDynamicCall {
    dispatch: TargetDynamicDispatch,
    call_plan: CallPlan,
    result_home: Option<TargetUnitScalarHomeRequirement>,
}

/// Lower one call admitted by `is_dynamic_call` to its `CallDynamic` row. An
/// integer result home joins the known-integer map; the home is returned so
/// the sequencing owner can publish it.
#[allow(clippy::too_many_arguments)]
pub(in crate::lowering) fn lower_dynamic_call(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &StructuralTypeLookup<'_>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    dynamic_parameters: &[TargetDynamicDescriptorParameterAbi],
    stored_descriptors: &BTreeSet<OperationId>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    scalar_values: &mut BTreeMap<ValueId, KnownUnitInteger>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<Option<TargetUnitScalarHomeRequirement>, LoweringError> {
    let (AbstractOperation::CallDynamic {
        psi_operation,
        requirement_obligations,
        crash_continuations,
        ..
    }
    | AbstractOperation::CallStructural {
        psi_operation,
        requirement_obligations,
        crash_continuations,
        ..
    }) = operation
    else {
        unreachable!("dynamic-call lowering receives only calls admitted by `is_dynamic_call`")
    };
    let lowered = match operation {
        AbstractOperation::CallDynamic {
            dispatch: AbstractDynamicDispatch::Rebound(custody),
            result,
            ..
        } => lower_rebound_dispatch(
            function,
            target,
            functions,
            structural_types,
            parameters_by_place,
            shape_cache,
            active,
            *psi_operation,
            custody,
            *result,
        )?,
        AbstractOperation::CallDynamic {
            dispatch: AbstractDynamicDispatch::Stored(custody),
            result,
            ..
        } => lower_stored_dispatch(
            function,
            target,
            functions,
            structural_types,
            parameters_by_place,
            stored_descriptors,
            shape_cache,
            active,
            *psi_operation,
            custody,
            *result,
        )?,
        AbstractOperation::CallDynamic {
            dispatch: AbstractDynamicDispatch::Parameter(custody),
            result,
            ..
        } => parameter::lower(
            function,
            target,
            dynamic_parameters,
            *psi_operation,
            custody,
            *result,
        )?,
        _ => descriptor_arguments::lower(
            operation,
            function,
            target,
            functions,
            structural_types,
            parameters_by_place,
            dynamic_parameters,
            shape_cache,
            active,
        )?,
    };
    let LoweredDynamicCall {
        dispatch,
        call_plan,
        result_home,
    } = lowered;
    record_integer_result_home(scalar_values, result_home)?;
    operations.push(TargetUnitOperation::CallDynamic {
        psi_operation: *psi_operation,
        dispatch,
        call_plan,
        result: result_home.map_or(
            TargetDynamicCallResult::Unit,
            TargetDynamicCallResult::Scalar,
        ),
        requirement_obligations: requirement_obligations.clone(),
        crash_continuations: crash_continuations.clone(),
    });
    provenance.operations.push(*psi_operation);
    Ok(result_home)
}

/// The durable home one scalar descriptor-call result defines, after
/// admitting only Boolean and integer results.
fn scalar_result_home(
    psi_operation: OperationId,
    result: AbstractResult,
) -> Result<TargetUnitScalarHomeRequirement, LoweringError> {
    if !matches!(
        result.scalar_type,
        ScalarType::Boolean | ScalarType::Integer(_)
    ) {
        return Err(LoweringError::UnitScalarCallIntegerTypeUnsupported(
            result.value,
        ));
    }
    Ok(TargetUnitScalarHomeRequirement {
        defining_operation: psi_operation,
        source_value: result.value,
        scalar_type: result.scalar_type,
        shape: scalar_shape(result.value, result.scalar_type, false)?,
    })
}

/// The scalar home one descriptor call's result role defines. A Unit result
/// defines none.
fn dynamic_result_home(
    psi_operation: OperationId,
    result: AbstractDynamicCallResult,
) -> Result<Option<TargetUnitScalarHomeRequirement>, LoweringError> {
    result
        .scalar()
        .map(|result| scalar_result_home(psi_operation, result))
        .transpose()
}

/// Publish an integer result home to the producer's known-integer map.
fn record_integer_result_home(
    scalar_values: &mut BTreeMap<ValueId, KnownUnitInteger>,
    result_home: Option<TargetUnitScalarHomeRequirement>,
) -> Result<(), LoweringError> {
    if let Some(result_home) = result_home
        && matches!(result_home.scalar_type, ScalarType::Integer(_))
    {
        insert_known_unit_integer(
            scalar_values,
            result_home.source_value,
            KnownUnitInteger::Home(result_home),
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(in crate::lowering) fn lower_stored_descriptor(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &StructuralTypeLookup<'_>,
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
    let calls = function
        .operations
        .iter()
        .filter_map(|candidate| match candidate {
            AbstractOperation::CallDynamic {
                psi_operation,
                dispatch: AbstractDynamicDispatch::Stored(dynamic_dispatch),
                result: AbstractDynamicCallResult::Scalar(result),
                ..
            } if &dynamic_dispatch.stored == stored => {
                Some((*psi_operation, dynamic_dispatch, result))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [(call_operation, dispatch, result)] = calls.as_slice() else {
        return Err(invalid());
    };
    let result_shape = scalar_shape(result.value, result.scalar_type, false)?;
    let (_, source_argument) = lower_stored_call(
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
        source_argument,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

/// Lower one call through a stored descriptor. Stored custody is admitted
/// only with a scalar result; stage 00 never produces a stored Unit call.
#[allow(clippy::too_many_arguments)]
fn lower_stored_dispatch(
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &StructuralTypeLookup<'_>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    stored_descriptors: &BTreeSet<OperationId>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    psi_operation: OperationId,
    custody: &AbstractStoredDynamicDispatch,
    result: AbstractDynamicCallResult,
) -> Result<LoweredDynamicCall, LoweringError> {
    let invalid = || LoweringError::InvalidDynamicDispatch {
        machine: function.machine,
        operation: psi_operation,
    };
    let AbstractDynamicCallResult::Scalar(result) = result else {
        return Err(invalid());
    };
    let result_home = scalar_result_home(psi_operation, result)?;
    // The sequencing owner supplies availability: an earlier flat operation or
    // a dominating graph definition. Authored block order is not execution order.
    let store_available =
        stored_descriptors.contains(&custody.stored.descriptor.establishment_operation);
    let store_count = function
        .operations
        .iter()
        .filter(|candidate| {
            matches!(candidate,
                AbstractOperation::StoreDynamicDescriptor { stored, .. }
                    if stored == &custody.stored)
        })
        .count();
    if !store_available || store_count != 1 {
        return Err(invalid());
    }
    let (call_plan, source_argument) = lower_stored_call(
        function,
        target,
        functions,
        structural_types,
        parameters_by_place,
        shape_cache,
        active,
        psi_operation,
        custody,
        result_home.scalar_type,
        result_home.shape,
    )?;
    Ok(LoweredDynamicCall {
        dispatch: TargetDynamicDispatch::Stored {
            custody: custody.clone(),
            source_argument,
        },
        call_plan,
        result_home: Some(result_home),
    })
}

/// The realization plan and projected source argument shared by a descriptor
/// store and its unique stored call.
#[allow(clippy::too_many_arguments)]
fn lower_stored_call(
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &StructuralTypeLookup<'_>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    psi_operation: OperationId,
    dynamic_dispatch: &AbstractStoredDynamicDispatch,
    expected_result: ScalarType,
    result_shape: ValueShape,
) -> Result<(CallPlan, TargetStructuralArgument), LoweringError> {
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
    let call_plan = StructuralCallSignature::derive(
        &[],
        &callee_function.structural_parameters,
        Some(result_shape),
        structural_types,
        shape_cache,
        active,
    )?
    .plan(target)?;
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
    Ok((call_plan, source_argument))
}

/// Lower one call through a rebound descriptor. Both result roles share the
/// callee join and the two projected selection arguments; the role selects
/// only the plan's result placement and whether a scalar home is defined.
#[allow(clippy::too_many_arguments)]
fn lower_rebound_dispatch(
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &StructuralTypeLookup<'_>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    psi_operation: OperationId,
    custody: &AbstractReboundDynamicDispatch,
    result: AbstractDynamicCallResult,
) -> Result<LoweredDynamicCall, LoweringError> {
    let result_home = dynamic_result_home(psi_operation, result)?;
    let expected_result = result_home.map(|home| home.scalar_type);
    let result_shape = result_home.map(|home| home.shape);
    let invalid = || LoweringError::InvalidDynamicDispatch {
        machine: function.machine,
        operation: psi_operation,
    };
    if function.attachment.is_none()
        || !custody.has_complete_application_custody(function.machine, psi_operation)
    {
        return Err(invalid());
    }
    let callee = custody.dispatch.realization;
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
    let call_plan = StructuralCallSignature::derive(
        &[],
        &callee_function.structural_parameters,
        result_shape,
        structural_types,
        shape_cache,
        active,
    )?
    .plan(target)?;
    let [destination] = call_plan.parameters.as_slice() else {
        return Err(LoweringError::UnitCallTargetKindMismatch(callee));
    };
    if call_plan.result.as_ref().map(|placement| placement.shape) != result_shape {
        return Err(LoweringError::UnitCallTargetKindMismatch(callee));
    }
    let initial_argument = projected_argument::lower(
        function.machine,
        callee,
        &custody.initial.source,
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
        &custody.rebound.source,
        callee_parameter,
        destination,
        structural_types,
        parameters_by_place,
        shape_cache,
        active,
    )?;
    Ok(LoweredDynamicCall {
        dispatch: TargetDynamicDispatch::Rebound {
            custody: custody.clone(),
            initial_argument,
            rebound_argument,
        },
        call_plan,
        result_home,
    })
}
