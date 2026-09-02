use super::setup::PreparedScalarLowering;
use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_special_form(
    function: &AbstractFunction,
    function_result: AbstractResult,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    prepared: &PreparedScalarLowering,
) -> Result<Option<TargetFunction>, LoweringError> {
    let mut shape_cache = prepared.shape_cache.clone();
    let mut active = prepared.active_shapes.clone();
    let call_plan = prepared.call_plan.clone();
    let target_structural_parameters = prepared.target_structural_parameters.clone();
    if let Some(lowered) = lower_dynamic_parameter_return(function, function_result, target)? {
        return Ok(Some(lowered));
    }
    if let Some(lowered) = structural_call::lower_direct_return(
        function,
        function_result,
        target,
        functions,
        structural_types,
        &call_plan,
        &target_structural_parameters,
        &mut shape_cache,
        &mut active,
    )? {
        return Ok(Some(lowered));
    }

    if let [
        AbstractOperation::BoundaryCall {
            psi_operation,
            result: Some(boundary_result),
            boundary,
            arguments: _,
            structural_arguments,
            completion_claim_sources,
            completion_receipts,
        },
        AbstractOperation::Return {
            psi_edge,
            result,
            value,
            scalar_type,
            cleanup_actions,
        },
    ] = function.operations.as_slice()
        && *result == function_result.value
        && *value == boundary_result.value
        && *scalar_type == boundary_result.scalar_type
        && boundary_result.scalar_type
            == ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 is a valid integer type"),
            )
        && cleanup_actions.is_empty()
        && structural_arguments
            .iter()
            .all(|argument| argument.path.is_empty())
    {
        let binding = settlements.get(boundary).cloned().ok_or(
            LoweringError::ResultBearingBoundarySettlementRequiresNativeRealization {
                machine: function.machine,
                operation: *psi_operation,
                boundary: *boundary,
            },
        )?;
        let omega_target_operations::BoundarySettlementRealization::Builtin(
            BoundaryRealization::DirectPortReadU8(realization),
        ) = binding.realization
        else {
            return Err(
                LoweringError::ResultBearingBoundarySettlementRequiresNativeRealization {
                    machine: function.machine,
                    operation: *psi_operation,
                    boundary: *boundary,
                },
            );
        };
        if target.architecture != Architecture::X86_64 {
            return Err(
                LoweringError::ResultBearingBoundarySettlementRequiresNativeRealization {
                    machine: function.machine,
                    operation: *psi_operation,
                    boundary: *boundary,
                },
            );
        }
        return Ok(Some(TargetFunction {
            machine: function.machine,
            attachment: function.attachment,
            fixed_integer_scalar_abi: None,
            mixed_structural_scalar_abi: None,
            provenance: TerminalPsiProvenance {
                operations: vec![*psi_operation],
                edges: vec![*psi_edge],
            },
            operation: TargetOperation::ReturnBoundaryPortReadU8 {
                psi_edge: *psi_edge,
                psi_operation: *psi_operation,
                source_value: boundary_result.value,
                boundary: *boundary,
                execution: binding.execution,
                realization,
                arguments: structural_arguments.clone(),
                completion_claim_sources: completion_claim_sources.clone(),
                completion_receipts: completion_receipts.clone(),
                call_plan,
                structural_parameters: target_structural_parameters,
            },
        }));
    }

    Ok(None)
}

fn lower_dynamic_parameter_return(
    function: &AbstractFunction,
    function_result: AbstractResult,
    target: NativeTarget,
) -> Result<Option<TargetFunction>, LoweringError> {
    let [
        AbstractOperation::DynamicDescriptorParameter { parameter },
        AbstractOperation::CallDynamicParameterScalar {
            psi_operation,
            result,
            dynamic_dispatch,
            requirement_obligations,
            crash_continuations,
        },
        AbstractOperation::Return {
            psi_edge,
            result: returned_result,
            value,
            scalar_type,
            cleanup_actions,
        },
    ] = function.operations.as_slice()
    else {
        return Ok(None);
    };
    let dispatch = &dynamic_dispatch.dispatch;
    let Some(requirement) = parameter
        .requirements
        .iter()
        .find(|requirement| requirement.slot == dispatch.requirement_slot)
        .cloned()
    else {
        return Err(LoweringError::InvalidDynamicScalarDispatch {
            machine: function.machine,
            operation: *psi_operation,
        });
    };
    if function.attachment.is_some()
        || !function.parameters.is_empty()
        || !function.structural_parameters.is_empty()
        || parameter != &dynamic_dispatch.parameter
        || parameter.owner != function.machine
        || parameter.ordinal != 0
        || parameter.access != psi_terminal::StructuralAccess::SharedBorrow
        || dispatch.owner != function.machine
        || dispatch.operation != *psi_operation
        || dispatch.parameter_ordinal != parameter.ordinal
        || *returned_result != function_result.value
        || *value != result.value
        || *scalar_type != result.scalar_type
        || !requirement_obligations.is_empty()
        || !crash_continuations.is_empty()
        || !cleanup_actions.is_empty()
        || !function.published_service_ceiling.is_empty()
    {
        return Err(LoweringError::InvalidDynamicScalarDispatch {
            machine: function.machine,
            operation: *psi_operation,
        });
    }
    let pointer_size = u16::try_from(target.pointer_size).map_err(|_| {
        LoweringError::InvalidDynamicScalarDispatch {
            machine: function.machine,
            operation: *psi_operation,
        }
    })?;
    let pointer_alignment = u16::try_from(target.pointer_alignment).map_err(|_| {
        LoweringError::InvalidDynamicScalarDispatch {
            machine: function.machine,
            operation: *psi_operation,
        }
    })?;
    let pointer_shape = ValueShape::integer(pointer_size, pointer_alignment);
    let result_shape = scalar_shape(result.value, result.scalar_type, false)?;
    let function_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![pointer_shape, pointer_shape],
            result: Some(result_shape),
        },
    )
    .map_err(LoweringError::AbiPlan)?;
    let [instance, table] = function_call_plan.parameters.as_slice() else {
        return Err(LoweringError::InvalidDynamicScalarDispatch {
            machine: function.machine,
            operation: *psi_operation,
        });
    };
    let dispatch_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![pointer_shape],
            result: Some(result_shape),
        },
    )
    .map_err(LoweringError::AbiPlan)?;
    let pointer_size_u32 = u32::try_from(target.pointer_size).map_err(|_| {
        LoweringError::InvalidDynamicScalarDispatch {
            machine: function.machine,
            operation: *psi_operation,
        }
    })?;
    let table_slot_byte_offset = dispatch
        .requirement_slot
        .checked_mul(pointer_size_u32)
        .ok_or(LoweringError::InvalidDynamicScalarDispatch {
            machine: function.machine,
            operation: *psi_operation,
        })?;
    Ok(Some(TargetFunction {
        machine: function.machine,
        attachment: function.attachment,
        fixed_integer_scalar_abi: None,
        mixed_structural_scalar_abi: None,
        provenance: TerminalPsiProvenance {
            operations: vec![*psi_operation],
            edges: vec![*psi_edge],
        },
        operation: TargetOperation::ReturnDynamicParameterScalarCall {
            psi_edge: *psi_edge,
            psi_operation: *psi_operation,
            source_value: result.value,
            scalar_type: result.scalar_type,
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
