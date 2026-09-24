//! Indirect calls through the function's own borrowed descriptor parameter
//! derive their contract once and share it between the source projection and
//! the target-custody replay: the semantic join, the signature roster binding,
//! the selected requirement row, the erased one-pointer adapter plan, and the
//! slot offset all recompute here from the roster, the dispatch, and the
//! target ABI. Either side substituting a stored row is a custody mismatch.
use super::scalar_shape;
use crate::LegalizationError;
use abstract_operations::{AbstractOperation, AbstractParameterDynamicDispatch};
use calling_conventions::{CallPlan, CallSignature, CallingPolicy, ValueShape};
use semantic_vocabulary::{IntegerSign, IntegerType, MachineId, OperationId, ScalarType};
use target::NativeTarget;
use target_operations::TargetDynamicDescriptorParameterAbi;
use terminal_psi::{ClosedConformanceCallableResult, TerminalDynamicRequirement};

/// A `DynamicDescriptorParameter` row declares signature custody only: it
/// carries no `OperationId`, so its node retains no provenance, fuel,
/// definition, use, successor, or ownership rows. Anything more is a
/// fabricated declaration and fails source custody.
pub(in crate::legalization) fn is_descriptor_declaration(
    node: &optimization_unit::OptimizationNode,
) -> bool {
    matches!(
        node.operation,
        AbstractOperation::DynamicDescriptorParameter { .. }
    ) && node.provenance.is_empty()
        && node.fuel.is_empty()
        && node.definitions.is_empty()
        && node.uses.is_empty()
        && node.successors.is_empty()
        && node.ownership.is_empty()
}

/// The independently recomputed contract of one parameter-descriptor call:
/// which signature-bound `{instance, table}` pair feeds it, which closed
/// interface row it invokes, the erased adapter plan for that row's result
/// shape, and the entry's byte offset in the incoming table.
pub(in crate::legalization) struct ParameterDynamicCallContract {
    pub parameter_abi: TargetDynamicDescriptorParameterAbi,
    pub requirement: TerminalDynamicRequirement,
    pub dispatch_call_plan: CallPlan,
    pub table_slot_byte_offset: u32,
}

pub(in crate::legalization) fn closed_result_scalar(
    result: ClosedConformanceCallableResult,
) -> Option<ScalarType> {
    match result {
        ClosedConformanceCallableResult::Unit => None,
        ClosedConformanceCallableResult::I32 => Some(ScalarType::Integer(
            IntegerType::new(IntegerSign::Signed, 32).expect("closed i32 result is valid"),
        )),
        ClosedConformanceCallableResult::Bool => Some(ScalarType::Boolean),
    }
}

pub(in crate::legalization) fn parameter_call_contract(
    dynamic_parameters: &[TargetDynamicDescriptorParameterAbi],
    machine: MachineId,
    psi_operation: OperationId,
    dynamic_dispatch: &AbstractParameterDynamicDispatch,
    expected_result: Option<ScalarType>,
    target: NativeTarget,
) -> Result<ParameterDynamicCallContract, LegalizationError> {
    let invalid = || LegalizationError::custody();
    // The semantic join: the dispatch names this operation, one exact borrowed
    // descriptor parameter of the same owner, and a requirement slot that
    // selects exactly one closed-interface row.
    if !dynamic_dispatch.has_complete_custody(machine, psi_operation) {
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
    let result_shape = expected_result.and_then(scalar_shape);
    let dispatch_call_plan = calling_conventions::evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![ValueShape::integer(pointer_size, pointer_alignment)],
            result: result_shape,
        },
    )
    .map_err(|_| invalid())?;
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
    Ok(ParameterDynamicCallContract {
        parameter_abi: parameter_abi.clone(),
        requirement: requirement.clone(),
        dispatch_call_plan,
        table_slot_byte_offset,
    })
}
