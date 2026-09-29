//! Indirect requirement invocation through a function's own descriptor
//! parameter: the descriptor's `{instance, table}` ABI placements supply the
//! receiver word and the slot-offset table word, and no realization machine is
//! ever selected statically.

use super::{LoweredDynamicCall, dynamic_result_home};
use crate::LoweringError;
use abstract_operations::{
    AbstractDynamicCallResult, AbstractFunction, AbstractParameterDynamicDispatch,
};
use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use semantic_vocabulary::{IntegerSign, IntegerType, OperationId, ScalarType};
use target::NativeTarget;
use target_operations::{TargetDynamicDescriptorParameterAbi, TargetDynamicDispatch};
use terminal_psi::ClosedConformanceCallableResult;

fn closed_result_scalar(result: ClosedConformanceCallableResult) -> Option<ScalarType> {
    match result {
        ClosedConformanceCallableResult::Unit => None,
        ClosedConformanceCallableResult::I32 => Some(ScalarType::Integer(
            IntegerType::new(IntegerSign::Signed, 32).expect("closed i32 result is valid"),
        )),
        ClosedConformanceCallableResult::Bool => Some(ScalarType::Boolean),
    }
}

/// Lower one call through a descriptor parameter. Both result roles share the
/// semantic and physical joins; the role selects only the erased plan's result
/// placement and whether a scalar home is defined.
pub(super) fn lower(
    function: &AbstractFunction,
    target: NativeTarget,
    dynamic_parameters: &[TargetDynamicDescriptorParameterAbi],
    psi_operation: OperationId,
    custody: &AbstractParameterDynamicDispatch,
    result: AbstractDynamicCallResult,
) -> Result<LoweredDynamicCall, LoweringError> {
    let result_home = dynamic_result_home(psi_operation, result)?;
    let expected_result = result_home.map(|home| home.scalar_type);
    let result_shape = result_home.map(|home| home.shape);
    let invalid = || LoweringError::InvalidDynamicDispatch {
        machine: function.machine,
        operation: psi_operation,
    };
    // The semantic join: the dispatch names this operation, one exact borrowed
    // descriptor parameter of the same owner, and a requirement slot that
    // selects exactly one closed-interface row.
    if !custody.has_complete_custody(function.machine, psi_operation) {
        return Err(invalid());
    }
    // The physical join: the consumed parameter must be one of the
    // signature-bound descriptor ABIs so the indirect call reads the exact
    // incoming `{instance, table}` placements rather than a substitute pair.
    let Some(parameter_abi) = dynamic_parameters
        .iter()
        .find(|abi| abi.parameter == custody.parameter)
    else {
        return Err(invalid());
    };
    let Some(requirement) = custody
        .parameter
        .requirements
        .iter()
        .find(|requirement| requirement.slot == custody.dispatch.requirement_slot)
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
    let table_slot_byte_offset = custody
        .dispatch
        .requirement_slot
        .checked_mul(u32::from(pointer_size))
        .ok_or_else(invalid)?;
    Ok(LoweredDynamicCall {
        dispatch: TargetDynamicDispatch::Parameter {
            custody: custody.clone(),
            parameter_abi: parameter_abi.clone(),
            requirement: requirement.clone(),
            table_slot_byte_offset,
        },
        call_plan: dispatch_call_plan,
        result_home,
    })
}
