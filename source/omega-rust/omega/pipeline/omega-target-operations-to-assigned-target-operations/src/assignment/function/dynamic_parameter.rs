//! Assignment of one forwarded existential descriptor and its slot call.

use crate::assignment::shared::*;

pub(super) fn assign(
    function: &TargetFunction,
    operation: &TargetOperation,
    target: NativeTarget,
) -> Result<AssignedOperation, AssignmentError> {
    if matches!(
        operation,
        TargetOperation::ReturnForwardedDynamicParameterScalarCall { .. }
            | TargetOperation::ForwardDynamicParameterUnitCall { .. }
    ) {
        return assign_forwarded(function, operation, target);
    }
    let (
        psi_edge,
        psi_operation,
        scalar_result,
        parameter_abi,
        requirement,
        function_call_plan,
        dispatch_call_plan,
        table_slot_byte_offset,
    ) = match operation {
        TargetOperation::ReturnDynamicParameterScalarCall {
            psi_edge,
            psi_operation,
            source_value,
            scalar_type,
            parameter_abi,
            requirement,
            function_call_plan,
            dispatch_call_plan,
            table_slot_byte_offset,
        } => (
            *psi_edge,
            *psi_operation,
            Some((*source_value, *scalar_type)),
            parameter_abi,
            requirement,
            function_call_plan,
            dispatch_call_plan,
            *table_slot_byte_offset,
        ),
        TargetOperation::DynamicParameterUnitCall {
            psi_edge,
            psi_operation,
            parameter_abi,
            requirement,
            function_call_plan,
            dispatch_call_plan,
            table_slot_byte_offset,
        } => (
            *psi_edge,
            *psi_operation,
            None,
            parameter_abi,
            requirement,
            function_call_plan,
            dispatch_call_plan,
            *table_slot_byte_offset,
        ),
        _ => unreachable!("dynamic-parameter assignment receives only its exact target role"),
    };
    let invalid = || AssignmentError::DynamicDescriptorAssignmentMismatch {
        machine: function.machine,
        operation: psi_operation,
    };
    let pointer_size = u16::try_from(target.pointer_size).map_err(|_| invalid())?;
    let pointer_alignment = u16::try_from(target.pointer_alignment).map_err(|_| invalid())?;
    let pointer_shape = ValueShape::integer(pointer_size, pointer_alignment);
    let result_shape = function_call_plan
        .result
        .as_ref()
        .map(|placement| placement.shape);
    let result_matches = match (requirement.result, scalar_result, result_shape) {
        (
            psi_terminal::ClosedConformanceCallableResult::I32,
            Some((_, psi_core::ScalarType::Integer(integer))),
            Some(result_shape),
        ) => {
            integer.carrier() == psi_core::IntegerCarrier::Fixed
                && integer.sign() == psi_core::IntegerSign::Signed
                && integer.bits() == 32
                && result_shape == ValueShape::integer(4, 4)
        }
        (
            psi_terminal::ClosedConformanceCallableResult::Bool,
            Some((_, psi_core::ScalarType::Boolean)),
            Some(result_shape),
        ) => result_shape == ValueShape::integer(1, 1),
        (psi_terminal::ClosedConformanceCallableResult::Unit, None, None) => true,
        _ => false,
    };
    let function_signature = CallSignature {
        parameters: vec![pointer_shape, pointer_shape],
        result: result_shape,
    };
    let dispatch_signature = CallSignature {
        parameters: vec![pointer_shape],
        result: result_shape,
    };
    if !result_matches
        || function_call_plan.policy != CallingPolicy::native_for_target(target)
        || dispatch_call_plan.policy != CallingPolicy::native_for_target(target)
        || omega_calling_conventions::validate_call_plan(function_call_plan, &function_signature)
            .is_err()
        || omega_calling_conventions::validate_call_plan(dispatch_call_plan, &dispatch_signature)
            .is_err()
        || function_call_plan.parameters.as_slice()
            != [parameter_abi.instance.clone(), parameter_abi.table.clone()]
        || parameter_abi.parameter.owner != function.machine
        || parameter_abi.parameter.ordinal != 0
        || parameter_abi
            .parameter
            .requirements
            .get(usize::try_from(requirement.slot).map_err(|_| invalid())?)
            != Some(requirement)
        || requirement.slot.checked_mul(u32::from(pointer_size)) != Some(table_slot_byte_offset)
        || function_call_plan.result != dispatch_call_plan.result
    {
        return Err(invalid());
    }
    let instance =
        pointer_register(&parameter_abi.instance, target.architecture).ok_or_else(invalid)?;
    let table = pointer_register(&parameter_abi.table, target.architecture).ok_or_else(invalid)?;
    let dispatch_instance = pointer_register(
        dispatch_call_plan.parameters.first().ok_or_else(invalid)?,
        target.architecture,
    )
    .ok_or_else(invalid)?;
    if instance != dispatch_instance || instance == table {
        return Err(invalid());
    }
    let mechanism = match target.architecture {
        Architecture::X86_64 => AssignedDynamicParameterCallMechanism::X86MemoryIndirect { table },
        Architecture::Aarch64 => AssignedDynamicParameterCallMechanism::Aarch64LoadedIndirect {
            table,
            target: MachineRegister::Aarch64X(16),
        },
    };
    let parameter_abi = AssignedDynamicDescriptorParameterAbi {
        parameter: parameter_abi.parameter.clone(),
        instance,
        table,
    };
    Ok(if let Some((source_value, scalar_type)) = scalar_result {
        AssignedOperation::ReturnDynamicParameterScalarCall {
            psi_edge,
            psi_operation,
            source_value,
            scalar_type,
            parameter_abi,
            requirement: requirement.clone(),
            function_call_plan: function_call_plan.clone(),
            dispatch_call_plan: dispatch_call_plan.clone(),
            table_slot_byte_offset,
            mechanism,
        }
    } else {
        AssignedOperation::DynamicParameterUnitCall {
            psi_edge,
            psi_operation,
            parameter_abi,
            requirement: requirement.clone(),
            function_call_plan: function_call_plan.clone(),
            dispatch_call_plan: dispatch_call_plan.clone(),
            table_slot_byte_offset,
            mechanism,
        }
    })
}

fn assign_forwarded(
    function: &TargetFunction,
    operation: &TargetOperation,
    target: NativeTarget,
) -> Result<AssignedOperation, AssignmentError> {
    let (
        psi_edge,
        psi_operation,
        scalar_result,
        callee,
        argument,
        parameter_abi,
        function_call_plan,
        callee_call_plan,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
    ) = match operation {
        TargetOperation::ReturnForwardedDynamicParameterScalarCall {
            psi_edge,
            psi_operation,
            source_value,
            scalar_type,
            callee,
            argument,
            parameter_abi,
            function_call_plan,
            callee_call_plan,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => (
            *psi_edge,
            *psi_operation,
            Some((*source_value, *scalar_type)),
            *callee,
            argument,
            parameter_abi,
            function_call_plan,
            callee_call_plan,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        ),
        TargetOperation::ForwardDynamicParameterUnitCall {
            psi_edge,
            psi_operation,
            callee,
            argument,
            parameter_abi,
            function_call_plan,
            callee_call_plan,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => (
            *psi_edge,
            *psi_operation,
            None,
            *callee,
            argument,
            parameter_abi,
            function_call_plan,
            callee_call_plan,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        ),
        _ => {
            unreachable!("forwarded dynamic-parameter assignment receives its exact target role")
        }
    };
    let invalid = || AssignmentError::DynamicDescriptorAssignmentMismatch {
        machine: function.machine,
        operation: psi_operation,
    };
    let result_shape = scalar_result.map(|(_, scalar_type)| match scalar_type {
        psi_core::ScalarType::Boolean => ValueShape::integer(1, 1),
        psi_core::ScalarType::Integer(integer) => ValueShape::integer(
            integer.bits().div_ceil(8),
            integer.bits().div_ceil(8).next_power_of_two().min(8),
        ),
        psi_core::ScalarType::IeeeFloat(psi_core::IeeeFloatFormat::Binary32) => {
            ValueShape::float(4)
        }
        psi_core::ScalarType::IeeeFloat(psi_core::IeeeFloatFormat::Binary64) => {
            ValueShape::float(8)
        }
    });
    let pointer_size = u16::try_from(target.pointer_size).map_err(|_| invalid())?;
    let pointer_alignment = u16::try_from(target.pointer_alignment).map_err(|_| invalid())?;
    let pointer_shape = ValueShape::integer(pointer_size, pointer_alignment);
    let signature = CallSignature {
        parameters: vec![pointer_shape, pointer_shape],
        result: result_shape,
    };
    if function.attachment.is_some()
        || function.fixed_integer_scalar_abi.is_some()
        || function.mixed_structural_scalar_abi.is_some()
        || parameter_abi.parameter.owner != function.machine
        || parameter_abi.parameter.ordinal != 0
        || argument.target.owner != callee
        || argument.target.ordinal != 0
        || !argument.has_complete_custody(function.machine, psi_operation, callee)
        || !matches!(
            &argument.source,
            omega_target_operations::AbstractDynamicDescriptorSource::Parameter(source)
                if source == &parameter_abi.parameter
        )
        || function_call_plan.policy != CallingPolicy::native_for_target(target)
        || callee_call_plan.policy != CallingPolicy::native_for_target(target)
        || omega_calling_conventions::validate_call_plan(function_call_plan, &signature).is_err()
        || omega_calling_conventions::validate_call_plan(callee_call_plan, &signature).is_err()
        || function_call_plan.parameters.as_slice()
            != [parameter_abi.instance.clone(), parameter_abi.table.clone()]
        || function_call_plan.result != callee_call_plan.result
        || !claim_transfers.is_empty()
        || !requirement_obligations.is_empty()
        || !crash_continuations.is_empty()
    {
        return Err(invalid());
    }
    let instance =
        pointer_register(&parameter_abi.instance, target.architecture).ok_or_else(invalid)?;
    let table = pointer_register(&parameter_abi.table, target.architecture).ok_or_else(invalid)?;
    let instance_destination = pointer_register(
        callee_call_plan.parameters.first().ok_or_else(invalid)?,
        target.architecture,
    )
    .ok_or_else(invalid)?;
    let table_destination = pointer_register(
        callee_call_plan.parameters.get(1).ok_or_else(invalid)?,
        target.architecture,
    )
    .ok_or_else(invalid)?;
    if instance != instance_destination || table != table_destination || instance == table {
        return Err(invalid());
    }
    let parameter_abi = AssignedDynamicDescriptorParameterAbi {
        parameter: parameter_abi.parameter.clone(),
        instance,
        table,
    };
    Ok(if let Some((source_value, scalar_type)) = scalar_result {
        AssignedOperation::ReturnForwardedDynamicParameterScalarCall {
            psi_edge,
            psi_operation,
            source_value,
            scalar_type,
            callee,
            argument: argument.clone(),
            parameter_abi,
            instance_destination,
            table_destination,
            function_call_plan: function_call_plan.clone(),
            callee_call_plan: callee_call_plan.clone(),
            claim_transfers: claim_transfers.clone(),
            requirement_obligations: requirement_obligations.clone(),
            crash_continuations: crash_continuations.clone(),
        }
    } else {
        AssignedOperation::ForwardDynamicParameterUnitCall {
            psi_edge,
            psi_operation,
            callee,
            argument: argument.clone(),
            parameter_abi,
            instance_destination,
            table_destination,
            function_call_plan: function_call_plan.clone(),
            callee_call_plan: callee_call_plan.clone(),
            claim_transfers: claim_transfers.clone(),
            requirement_obligations: requirement_obligations.clone(),
            crash_continuations: crash_continuations.clone(),
        }
    })
}

fn pointer_register(
    placement: &omega_calling_conventions::ValuePlacement,
    architecture: Architecture,
) -> Option<MachineRegister> {
    let [
        omega_calling_conventions::ValueLocation::Register {
            register,
            value_byte_offset: 0,
            byte_size,
        },
    ] = placement.locations.as_slice()
    else {
        return None;
    };
    (*byte_size == 8 && register.architecture() == architecture).then_some(*register)
}
