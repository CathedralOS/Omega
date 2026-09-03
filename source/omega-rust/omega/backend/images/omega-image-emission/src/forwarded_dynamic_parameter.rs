//! Independent object-boundary replay for transparent descriptor-parameter helpers.

use omega_calling_conventions::{CallSignature, CallingPolicy, ValueLocation, ValueShape};
use omega_machine_code::{ForwardedDynamicParameterCallRecord, MachineCodeFunction};
use omega_target::{Architecture, NativeTarget};
use omega_target_operations::{CallSiteOwner, MachineRegister};
use psi_core::MachineId;

use super::{ObjectError, SemanticCodeSite};

pub(super) fn validate_forwarded_dynamic_parameter_calls(
    target: NativeTarget,
    functions: &[MachineCodeFunction],
) -> Result<(), ObjectError> {
    let by_machine = functions
        .iter()
        .map(|function| (function.machine, function))
        .collect::<std::collections::BTreeMap<_, _>>();
    for function in functions {
        if function.forwarded_dynamic_parameter_calls.len() > 1 {
            let operation = function
                .forwarded_dynamic_parameter_calls
                .first()
                .map_or_else(|| unreachable!(), |call| call.psi_operation);
            return Err(invalid(function.machine, operation));
        }
        for call in &function.forwarded_dynamic_parameter_calls {
            validate_call(target, function, call, &by_machine)?;
        }
    }
    Ok(())
}

fn validate_call(
    target: NativeTarget,
    function: &MachineCodeFunction,
    call: &ForwardedDynamicParameterCallRecord,
    functions: &std::collections::BTreeMap<MachineId, &MachineCodeFunction>,
) -> Result<(), ObjectError> {
    let invalid = || invalid(function.machine, call.psi_operation);
    let callee = functions.get(&call.callee).copied().ok_or_else(invalid)?;
    let (callee_parameter, callee_plan) = match (
        callee.forwarded_dynamic_parameter_calls.as_slice(),
        callee.dynamic_parameter_calls.as_slice(),
    ) {
        ([next], []) => (&next.parameter, &next.function_call_plan),
        ([], [dispatch]) => (&dispatch.parameter, &dispatch.function_call_plan),
        _ => return Err(invalid()),
    };
    let result_shape = scalar_shape(call.scalar_type);
    let pointer_size = u16::try_from(target.pointer_size).map_err(|_| invalid())?;
    let pointer_alignment = u16::try_from(target.pointer_alignment).map_err(|_| invalid())?;
    let signature = CallSignature {
        parameters: vec![
            ValueShape::integer(pointer_size, pointer_alignment),
            ValueShape::integer(pointer_size, pointer_alignment),
        ],
        result: Some(result_shape),
    };
    let [operation_attribution, return_attribution] = function.semantic_code_attribution.as_slice()
    else {
        return Err(invalid());
    };
    let [relocation] = function.internal_calls.as_slice() else {
        return Err(invalid());
    };
    let call_end = call
        .direct_call_offset
        .checked_add(call.direct_call_byte_count)
        .ok_or_else(invalid)?;
    let operation_end = call
        .code_offset
        .checked_add(call.byte_count)
        .ok_or_else(invalid)?;
    let return_byte_count = match target.architecture {
        Architecture::X86_64 => 1,
        Architecture::Aarch64 => 4,
    };
    let return_end = operation_end
        .checked_add(return_byte_count)
        .ok_or_else(invalid)?;
    let relocation_matches = match target.architecture {
        Architecture::X86_64 => {
            call.direct_call_byte_count == 5
                && relocation.offset == call.direct_call_offset + 1
                && function.bytes.get(call.direct_call_offset) == Some(&0xe8)
        }
        Architecture::Aarch64 => {
            call.direct_call_byte_count == 4
                && relocation.offset == call.direct_call_offset
                && function
                    .bytes
                    .get(call.direct_call_offset..call_end)
                    .and_then(|bytes| bytes.try_into().ok())
                    .map(u32::from_le_bytes)
                    == Some(0x9400_0000)
        }
    };
    if function.attachment.is_some()
        || function.unit_stack.is_some()
        || function.scalar_stack.is_none()
        || !function.dynamic_parameter_calls.is_empty()
        || !function.forwarded_dynamic_descriptor_calls.is_empty()
        || call.code_offset != 0
        || return_end != function.bytes.len()
        || call_end > operation_end
        || call.parameter.owner != function.machine
        || call.parameter.ordinal != 0
        || callee_parameter != &call.argument.target
        || callee_plan != &call.callee_call_plan
        || !call
            .argument
            .has_complete_custody(function.machine, call.psi_operation, call.callee)
        || !matches!(
            &call.argument.source,
            omega_target_operations::AbstractDynamicDescriptorSource::Parameter(source)
                if source == &call.parameter
        )
        || call.function_call_plan.policy != CallingPolicy::native_for_target(target)
        || call.callee_call_plan.policy != CallingPolicy::native_for_target(target)
        || omega_calling_conventions::validate_call_plan(&call.function_call_plan, &signature)
            .is_err()
        || omega_calling_conventions::validate_call_plan(&call.callee_call_plan, &signature)
            .is_err()
        || call.function_call_plan != call.callee_call_plan
        || exact_register(&call.function_call_plan.parameters[0]) != Some(call.instance)
        || exact_register(&call.function_call_plan.parameters[1]) != Some(call.table)
        || exact_register(&call.callee_call_plan.parameters[0]) != Some(call.instance_destination)
        || exact_register(&call.callee_call_plan.parameters[1]) != Some(call.table_destination)
        || call.instance != call.instance_destination
        || call.table != call.table_destination
        || call.instance == call.table
        || relocation.owner != CallSiteOwner::Operation(call.psi_operation)
        || relocation.target != call.callee
        || relocation.unit_stack.is_some()
        || relocation.scalar_stack.as_ref() != Some(&call.call_stack)
        || !relocation_matches
        || operation_attribution.site != SemanticCodeSite::Operation(call.psi_operation)
        || operation_attribution.operation_ordinal != call.operation_ordinal
        || operation_attribution.code_offset != call.code_offset
        || operation_attribution.byte_count != call.byte_count
        || return_attribution.site != SemanticCodeSite::Edge(call.psi_edge)
        || return_attribution.operation_ordinal != 1
        || return_attribution.code_offset != operation_end
        || return_attribution.byte_count != return_byte_count
        || function.provenance.operations.as_slice() != [call.psi_operation]
        || function.provenance.edges.as_slice() != [call.psi_edge]
    {
        return Err(invalid());
    }
    Ok(())
}

fn scalar_shape(scalar_type: psi_core::ScalarType) -> ValueShape {
    match scalar_type {
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
    }
}

fn exact_register(
    placement: &omega_calling_conventions::ValuePlacement,
) -> Option<MachineRegister> {
    let [
        ValueLocation::Register {
            register,
            value_byte_offset: 0,
            byte_size,
        },
    ] = placement.locations.as_slice()
    else {
        return None;
    };
    (*byte_size == placement.shape.byte_size).then_some(*register)
}

fn invalid(caller: MachineId, operation: psi_core::OperationId) -> ObjectError {
    ObjectError::InvalidForwardedDynamicParameterCallEvidence { caller, operation }
}
