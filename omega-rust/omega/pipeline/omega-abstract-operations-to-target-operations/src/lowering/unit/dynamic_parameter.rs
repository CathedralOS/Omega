//! Function-level lowering for one result-less descriptor-parameter dispatch.

use super::super::shared::*;

pub(super) fn lower(
    function: &AbstractFunction,
    target: NativeTarget,
) -> Result<Option<TargetFunction>, LoweringError> {
    let [
        AbstractOperation::DynamicDescriptorParameter { parameter },
        AbstractOperation::CallDynamicParameterUnit {
            psi_operation,
            dynamic_dispatch,
            requirement_obligations,
            crash_continuations,
        },
        AbstractOperation::ReturnUnit {
            psi_edge,
            cleanup_actions,
        },
    ] = function.operations.as_slice()
    else {
        return Ok(None);
    };
    let invalid = || LoweringError::InvalidDynamicDispatch {
        machine: function.machine,
        operation: *psi_operation,
    };
    let dispatch = &dynamic_dispatch.dispatch;
    let requirement = parameter
        .requirements
        .iter()
        .find(|requirement| requirement.slot == dispatch.requirement_slot)
        .cloned()
        .ok_or_else(invalid)?;
    if function.result != AbstractFunctionResult::Unit
        || function.attachment.is_some()
        || !function.parameters.is_empty()
        || !function.structural_parameters.is_empty()
        || parameter != &dynamic_dispatch.parameter
        || parameter.owner != function.machine
        || parameter.ordinal != 0
        || !matches!(
            parameter.access,
            psi_terminal::StructuralAccess::SharedBorrow
                | psi_terminal::StructuralAccess::MutableBorrow
        )
        || dispatch.owner != function.machine
        || dispatch.operation != *psi_operation
        || dispatch.parameter_ordinal != parameter.ordinal
        || requirement.result != psi_terminal::ClosedConformanceCallableResult::Unit
        || !requirement_obligations.is_empty()
        || !crash_continuations.is_empty()
        || !cleanup_actions.is_empty()
        || !function.published_service_ceiling.is_empty()
    {
        return Err(invalid());
    }
    let pointer_size = u16::try_from(target.pointer_size).map_err(|_| invalid())?;
    let pointer_alignment = u16::try_from(target.pointer_alignment).map_err(|_| invalid())?;
    let pointer_shape = ValueShape::integer(pointer_size, pointer_alignment);
    let policy = CallingPolicy::native_for_target(target);
    let function_call_plan = evaluate_call_plan(
        policy,
        &CallSignature {
            parameters: vec![pointer_shape, pointer_shape],
            result: None,
        },
    )
    .map_err(LoweringError::AbiPlan)?;
    let [instance, table] = function_call_plan.parameters.as_slice() else {
        return Err(invalid());
    };
    let dispatch_call_plan = evaluate_call_plan(
        policy,
        &CallSignature {
            parameters: vec![pointer_shape],
            result: None,
        },
    )
    .map_err(LoweringError::AbiPlan)?;
    let table_slot_byte_offset = dispatch
        .requirement_slot
        .checked_mul(u32::try_from(target.pointer_size).map_err(|_| invalid())?)
        .ok_or_else(invalid)?;
    Ok(Some(TargetFunction {
        machine: function.machine,
        attachment: function.attachment,
        fixed_integer_scalar_abi: None,
        mixed_structural_scalar_abi: None,
        provenance: TerminalPsiProvenance {
            operations: vec![*psi_operation],
            edges: vec![*psi_edge],
        },
        operation: TargetOperation::DynamicParameterUnitCall {
            psi_edge: *psi_edge,
            psi_operation: *psi_operation,
            parameter_abi: TargetDynamicDescriptorParameterAbi {
                parameter: parameter.clone(),
                instance: instance.clone(),
                table: table.clone(),
            },
            requirement,
            function_call_plan,
            dispatch_call_plan,
            table_slot_byte_offset,
        },
    }))
}
