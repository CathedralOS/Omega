//! Indirect requirement invocation through a function's own descriptor
//! parameter: the descriptor's `{instance, table}` ABI placements supply the
//! receiver word and the slot-offset table word, and no realization machine is
//! ever selected statically.

use super::super::scalar::scalar_shape;
use super::scalar_call::{KnownUnitInteger, insert_known_unit_integer};
use crate::LoweringError;
use abstract_operations::AbstractParameterDynamicDispatch;
use abstract_operations::{AbstractFunction, AbstractOperation};
use calling_conventions::{CallPlan, CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use semantic_vocabulary::{IntegerSign, IntegerType, OperationId, ScalarType, ValueId};
use std::collections::BTreeMap;
use target::NativeTarget;
use target_operations::{
    TargetDynamicDescriptorParameterAbi, TargetUnitOperation, TargetUnitScalarHomeRequirement,
    TerminalPsiProvenance,
};
use terminal_psi::{ClosedConformanceCallableResult, TerminalDynamicRequirement};

struct LoweredParameterDynamicCall {
    parameter_abi: TargetDynamicDescriptorParameterAbi,
    requirement: TerminalDynamicRequirement,
    dispatch_call_plan: CallPlan,
    table_slot_byte_offset: u32,
}

fn closed_result_scalar(result: ClosedConformanceCallableResult) -> Option<ScalarType> {
    match result {
        ClosedConformanceCallableResult::Unit => None,
        ClosedConformanceCallableResult::I32 => Some(ScalarType::Integer(
            IntegerType::new(IntegerSign::Signed, 32).expect("closed i32 result is valid"),
        )),
        ClosedConformanceCallableResult::Bool => Some(ScalarType::Boolean),
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_parameter_dynamic_call(
    function: &AbstractFunction,
    target: NativeTarget,
    dynamic_parameters: &[TargetDynamicDescriptorParameterAbi],
    psi_operation: OperationId,
    dynamic_dispatch: &AbstractParameterDynamicDispatch,
    expected_result: Option<ScalarType>,
    result_shape: Option<ValueShape>,
) -> Result<LoweredParameterDynamicCall, LoweringError> {
    let invalid = || LoweringError::InvalidDynamicDispatch {
        machine: function.machine,
        operation: psi_operation,
    };
    // The semantic join: the dispatch names this operation, one exact borrowed
    // descriptor parameter of the same owner, and a requirement slot that
    // selects exactly one closed-interface row.
    if !dynamic_dispatch.has_complete_custody(function.machine, psi_operation) {
        return Err(invalid());
    }
    // The physical join: the consumed parameter must be one of the
    // signature-bound descriptor ABIs so the indirect call reads the exact
    // incoming `{instance, table}` placements rather than a substitute pair.
    let Some(parameter_abi) = dynamic_parameters
        .iter()
        .find(|abi| abi.parameter == dynamic_dispatch.parameter)
    else {
        return Err(invalid());
    };
    let Some(requirement) = dynamic_dispatch
        .parameter
        .requirements
        .iter()
        .find(|requirement| requirement.slot == dynamic_dispatch.dispatch.requirement_slot)
    else {
        return Err(invalid());
    };
    if closed_result_scalar(requirement.result) != expected_result {
        return Err(invalid());
    }
    let pointer_size = u16::try_from(target.pointer_size).map_err(|_| invalid())?;
    let pointer_alignment = u16::try_from(target.pointer_alignment).map_err(|_| invalid())?;
    // The erased adapter entry receives exactly the instance word and returns
    // the requirement's result shape; the selected table slot must satisfy
    // this plan without naming a concrete realization.
    let dispatch_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![ValueShape::integer(pointer_size, pointer_alignment)],
            result: result_shape,
        },
    )
    .map_err(LoweringError::AbiPlan)?;
    if dispatch_call_plan.parameters.len() != 1
        || dispatch_call_plan
            .result
            .as_ref()
            .map(|placement| placement.shape)
            != result_shape
    {
        return Err(invalid());
    }
    let table_slot_byte_offset = dynamic_dispatch
        .dispatch
        .requirement_slot
        .checked_mul(u32::from(pointer_size))
        .ok_or_else(invalid)?;
    Ok(LoweredParameterDynamicCall {
        parameter_abi: parameter_abi.clone(),
        requirement: requirement.clone(),
        dispatch_call_plan,
        table_slot_byte_offset,
    })
}

pub(in crate::lowering) fn lower_parameter_dynamic_scalar_call(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    target: NativeTarget,
    dynamic_parameters: &[TargetDynamicDescriptorParameterAbi],
    scalar_values: &mut BTreeMap<ValueId, KnownUnitInteger>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<TargetUnitScalarHomeRequirement, LoweringError> {
    let AbstractOperation::CallDynamicParameterScalar {
        psi_operation,
        result,
        dynamic_dispatch,
        requirement_obligations,
        crash_continuations,
    } = operation
    else {
        unreachable!("parameter dynamic scalar lowering receives only its exact role")
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
    let lowered = lower_parameter_dynamic_call(
        function,
        target,
        dynamic_parameters,
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
    operations.push(TargetUnitOperation::DynamicParameterScalarCall {
        psi_operation: *psi_operation,
        result: *result,
        dynamic_dispatch: dynamic_dispatch.clone(),
        parameter_abi: lowered.parameter_abi,
        requirement: lowered.requirement,
        dispatch_call_plan: lowered.dispatch_call_plan,
        table_slot_byte_offset: lowered.table_slot_byte_offset,
        result_home,
        requirement_obligations: requirement_obligations.clone(),
        crash_continuations: crash_continuations.clone(),
    });
    provenance.operations.push(*psi_operation);
    Ok(result_home)
}

pub(in crate::lowering) fn lower_parameter_dynamic_unit_call(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    target: NativeTarget,
    dynamic_parameters: &[TargetDynamicDescriptorParameterAbi],
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::CallDynamicParameterUnit {
        psi_operation,
        dynamic_dispatch,
        requirement_obligations,
        crash_continuations,
    } = operation
    else {
        unreachable!("parameter dynamic Unit lowering receives only its exact role")
    };
    let lowered = lower_parameter_dynamic_call(
        function,
        target,
        dynamic_parameters,
        *psi_operation,
        dynamic_dispatch,
        None,
        None,
    )?;
    operations.push(TargetUnitOperation::DynamicParameterUnitCall {
        psi_operation: *psi_operation,
        dynamic_dispatch: dynamic_dispatch.clone(),
        parameter_abi: lowered.parameter_abi,
        requirement: lowered.requirement,
        dispatch_call_plan: lowered.dispatch_call_plan,
        table_slot_byte_offset: lowered.table_slot_byte_offset,
        requirement_obligations: requirement_obligations.clone(),
        crash_continuations: crash_continuations.clone(),
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}
