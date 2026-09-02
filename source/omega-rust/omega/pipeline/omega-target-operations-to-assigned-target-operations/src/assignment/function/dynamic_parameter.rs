//! Assignment of one forwarded existential descriptor and its slot call.

use crate::assignment::shared::*;

pub(super) fn assign(
    function: &TargetFunction,
    operation: &TargetOperation,
    target: NativeTarget,
) -> Result<AssignedOperation, AssignmentError> {
    let TargetOperation::ReturnDynamicParameterScalarCall {
        psi_edge,
        psi_operation,
        source_value,
        scalar_type,
        parameter_abi,
        requirement,
        function_call_plan,
        dispatch_call_plan,
        table_slot_byte_offset,
    } = operation
    else {
        unreachable!("dynamic-parameter assignment receives only its exact target role")
    };
    let invalid = || AssignmentError::DynamicDescriptorAssignmentMismatch {
        machine: function.machine,
        operation: *psi_operation,
    };
    let pointer_size = u16::try_from(target.pointer_size).map_err(|_| invalid())?;
    let pointer_alignment = u16::try_from(target.pointer_alignment).map_err(|_| invalid())?;
    let pointer_shape = ValueShape::integer(pointer_size, pointer_alignment);
    let result_shape = function_call_plan
        .result
        .as_ref()
        .map(|placement| placement.shape)
        .ok_or_else(invalid)?;
    let result_matches = match (requirement.result, scalar_type) {
        (
            psi_terminal::ClosedConformanceCallableResult::I32,
            psi_core::ScalarType::Integer(integer),
        ) => {
            integer.carrier() == psi_core::IntegerCarrier::Fixed
                && integer.sign() == psi_core::IntegerSign::Signed
                && integer.bits() == 32
                && result_shape == ValueShape::integer(4, 4)
        }
        (psi_terminal::ClosedConformanceCallableResult::Bool, psi_core::ScalarType::Boolean) => {
            result_shape == ValueShape::integer(1, 1)
        }
        _ => false,
    };
    let function_signature = CallSignature {
        parameters: vec![pointer_shape, pointer_shape],
        result: Some(result_shape),
    };
    let dispatch_signature = CallSignature {
        parameters: vec![pointer_shape],
        result: Some(result_shape),
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
        || requirement.slot.checked_mul(u32::from(pointer_size)) != Some(*table_slot_byte_offset)
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
    Ok(AssignedOperation::ReturnDynamicParameterScalarCall {
        psi_edge: *psi_edge,
        psi_operation: *psi_operation,
        source_value: *source_value,
        scalar_type: *scalar_type,
        parameter_abi: AssignedDynamicDescriptorParameterAbi {
            parameter: parameter_abi.parameter.clone(),
            instance,
            table,
        },
        requirement: requirement.clone(),
        function_call_plan: function_call_plan.clone(),
        dispatch_call_plan: dispatch_call_plan.clone(),
        table_slot_byte_offset: *table_slot_byte_offset,
        mechanism,
    })
}

fn pointer_register(
    placement: &omega_calling_conventions::ValuePlacement,
    architecture: Architecture,
) -> Option<MachineRegister> {
    let [omega_calling_conventions::ValueLocation::Register {
        register,
        value_byte_offset: 0,
        byte_size,
    }] = placement.locations.as_slice()
    else {
        return None;
    };
    (*byte_size == 8 && register.architecture() == architecture).then_some(*register)
}
