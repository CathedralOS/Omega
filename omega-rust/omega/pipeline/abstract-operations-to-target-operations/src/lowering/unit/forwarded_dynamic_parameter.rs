//! Function-level lowering for one result-less descriptor-parameter handoff.

use super::super::shared::*;

pub(super) fn lower(
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
) -> Result<Option<TargetFunction>, LoweringError> {
    let [
        AbstractOperation::DynamicDescriptorParameter { parameter },
        AbstractOperation::CallUnitWithDynamicArguments {
            psi_operation,
            callee,
            structural_arguments,
            dynamic_arguments,
            claim_transfers,
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
    let [argument] = dynamic_arguments.as_slice() else {
        return Err(invalid(function.machine, *psi_operation));
    };
    let AbstractDynamicDescriptorSource::Parameter(source_parameter) = &argument.source else {
        return Ok(None);
    };
    let invalid = || invalid(function.machine, *psi_operation);
    let callee_function = functions
        .get(callee)
        .copied()
        .ok_or(LoweringError::UnknownCallTarget(*callee))?;
    let Some(AbstractOperation::DynamicDescriptorParameter {
        parameter: callee_parameter,
    }) = callee_function.operations.first()
    else {
        return Err(invalid());
    };
    if function.result != AbstractFunctionResult::Unit
        || callee_function.result != AbstractFunctionResult::Unit
        || function.attachment.is_some()
        || callee_function.attachment.is_some()
        || !function.parameters.is_empty()
        || !function.structural_parameters.is_empty()
        || !callee_function.parameters.is_empty()
        || !callee_function.structural_parameters.is_empty()
        || parameter != source_parameter
        || parameter.owner != function.machine
        || parameter.ordinal != 0
        || !matches!(
            parameter.access,
            terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow
        )
        || callee_parameter != &argument.target
        || callee_parameter.owner != *callee
        || callee_parameter.ordinal != 0
        || !argument.has_complete_custody(function.machine, *psi_operation, *callee)
        || !structural_arguments.is_empty()
        || !claim_transfers.is_empty()
        || !requirement_obligations.is_empty()
        || !crash_continuations.is_empty()
        || !cleanup_actions.is_empty()
        || !function.published_service_ceiling.is_empty()
        || !callee_function.published_service_ceiling.is_empty()
    {
        return Err(invalid());
    }
    let pointer_size = u16::try_from(target.pointer_size).map_err(|_| invalid())?;
    let pointer_alignment = u16::try_from(target.pointer_alignment).map_err(|_| invalid())?;
    let pointer_shape = ValueShape::integer(pointer_size, pointer_alignment);
    let signature = CallSignature {
        parameters: vec![pointer_shape, pointer_shape],
        result: None,
    };
    let function_call_plan =
        evaluate_call_plan(CallingPolicy::native_for_target(target), &signature)
            .map_err(LoweringError::AbiPlan)?;
    let callee_call_plan = evaluate_call_plan(CallingPolicy::native_for_target(target), &signature)
        .map_err(LoweringError::AbiPlan)?;
    let [instance, table] = function_call_plan.parameters.as_slice() else {
        return Err(invalid());
    };
    Ok(Some(TargetFunction {
        machine: function.machine,
        attachment: None,
        fixed_integer_scalar_abi: None,
        mixed_structural_scalar_abi: None,
        provenance: TerminalPsiProvenance {
            operations: vec![*psi_operation],
            edges: vec![*psi_edge],
        },
        operation: TargetOperation::ForwardDynamicParameterUnitCall {
            psi_edge: *psi_edge,
            psi_operation: *psi_operation,
            callee: *callee,
            argument: argument.clone(),
            parameter_abi: TargetDynamicDescriptorParameterAbi {
                parameter: parameter.clone(),
                instance: instance.clone(),
                table: table.clone(),
            },
            function_call_plan,
            callee_call_plan,
            claim_transfers: claim_transfers.clone(),
            requirement_obligations: requirement_obligations.clone(),
            crash_continuations: crash_continuations.clone(),
        },
    }))
}

fn invalid(machine: MachineId, operation: OperationId) -> LoweringError {
    LoweringError::InvalidDynamicDispatch { machine, operation }
}
