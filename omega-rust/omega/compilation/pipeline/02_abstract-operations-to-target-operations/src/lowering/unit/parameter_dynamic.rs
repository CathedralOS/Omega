//! Indirect requirement invocation through a function's own descriptor
//! parameter: the descriptor's `{instance, table}` ABI placements supply the
//! receiver word and the slot-offset table word, and no realization machine is
//! ever selected statically.

use super::dynamic::{dynamic_result_home, record_integer_result_home};
use super::scalar_call::KnownUnitInteger;
use crate::LoweringError;
use abstract_operations::{
    AbstractDynamicDispatch, AbstractFunction, AbstractOperation, AbstractParameterDynamicDispatch,
};
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
fn lower_parameter_dispatch(
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
            parameters: vec![ValueShape::integer(
                u32::from(pointer_size),
                pointer_alignment,
            )],
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

/// Lower one call through a descriptor parameter. Both result roles share the
/// semantic and physical joins; the role selects only the target row and
/// whether a scalar home is defined.
pub(in crate::lowering) fn lower_parameter_dynamic_call(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    target: NativeTarget,
    dynamic_parameters: &[TargetDynamicDescriptorParameterAbi],
    scalar_values: &mut BTreeMap<ValueId, KnownUnitInteger>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<Option<TargetUnitScalarHomeRequirement>, LoweringError> {
    let AbstractOperation::CallDynamic {
        psi_operation,
        dispatch: AbstractDynamicDispatch::Parameter(dynamic_dispatch),
        result,
        requirement_obligations,
        crash_continuations,
    } = operation
    else {
        unreachable!("parameter dynamic lowering receives only parameter descriptor calls")
    };
    let scalar = dynamic_result_home(*psi_operation, *result)?;
    let result_home = scalar.map(|(_, home)| home);
    let lowered = lower_parameter_dispatch(
        function,
        target,
        dynamic_parameters,
        *psi_operation,
        dynamic_dispatch,
        result_home.map(|home| home.scalar_type),
        result_home.map(|home| home.shape),
    )?;
    record_integer_result_home(scalar_values, result_home)?;
    operations.push(match scalar {
        Some((result, result_home)) => TargetUnitOperation::DynamicParameterScalarCall {
            psi_operation: *psi_operation,
            result,
            dynamic_dispatch: dynamic_dispatch.clone(),
            parameter_abi: lowered.parameter_abi,
            requirement: lowered.requirement,
            dispatch_call_plan: lowered.dispatch_call_plan,
            table_slot_byte_offset: lowered.table_slot_byte_offset,
            result_home,
            requirement_obligations: requirement_obligations.clone(),
            crash_continuations: crash_continuations.clone(),
        },
        None => TargetUnitOperation::DynamicParameterUnitCall {
            psi_operation: *psi_operation,
            dynamic_dispatch: dynamic_dispatch.clone(),
            parameter_abi: lowered.parameter_abi,
            requirement: lowered.requirement,
            dispatch_call_plan: lowered.dispatch_call_plan,
            table_slot_byte_offset: lowered.table_slot_byte_offset,
            requirement_obligations: requirement_obligations.clone(),
            crash_continuations: crash_continuations.clone(),
        },
    });
    provenance.operations.push(*psi_operation);
    Ok(result_home)
}
