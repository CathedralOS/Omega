//! Caller-side materialization for forwarded existential descriptors.

use omega_assigned_target_operations::{
    AssignedDynamicDescriptorArgument, AssignedFunction, AssignedOperation, AssignedUnitOperation,
};
use omega_calling_conventions::{
    CallSignature, CallingPolicy, IndirectPointerLocation, ValueLocation, ValuePlacement,
    ValueShape, evaluate_call_plan,
};
use omega_machine_code::{
    DynamicTableAddressEncoding, DynamicTableAddressMaterialization,
    ForwardedDynamicDescriptorAdapterIdentity, ForwardedDynamicDescriptorAdapterRecord,
    ForwardedDynamicDescriptorArgumentRecord, ForwardedDynamicDescriptorCallRecord,
    InternalCallRelocation,
};
use omega_target::{Architecture, NativeTarget};
use omega_target_operations::{CallSiteOwner, TargetDynamicDescriptorInstanceArgument};

use super::scalar_call::emit_unit_scalar_result;
use super::{
    Aarch64UnitParameterHome, X86UnitParameterHome, emit_aarch64_unit_call, emit_x86_64_unit_call,
};
use crate::{
    EmissionError, aarch64_load_base, aarch64_store_base, aarch64_unit_memory_access,
    aarch64_unit_register, aarch64_unit_stack_access, append_aarch64_instructions,
    emit_aarch64_adjust_sp, emit_aarch64_sp_address, emit_x86_64_adjust_sp,
    emit_x86_64_memory_load_width, emit_x86_64_stack_load_width, x86_unit_register,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_forwarded_dynamic_descriptor_call(
    operation: &AssignedUnitOperation,
    owner: psi_core::MachineId,
    target: NativeTarget,
    functions: &[AssignedFunction],
    x86_homes: &[X86UnitParameterHome],
    aarch64_homes: &[Aarch64UnitParameterHome],
    bytes: &mut Vec<u8>,
    internal_calls: &mut Vec<InternalCallRelocation>,
    operation_ordinal: usize,
    code_offset: usize,
) -> Result<ForwardedDynamicDescriptorCallRecord, EmissionError> {
    let AssignedUnitOperation::StructuralScalarCallWithDynamicArguments {
        psi_operation,
        result,
        callee,
        call_plan,
        result_home,
        copies,
        dynamic_arguments,
        claim_transfers,
        ..
    } = operation
    else {
        unreachable!("forwarded descriptor router supplied another operation")
    };
    let invalid = || EmissionError::InvalidDynamicDescriptorCallCustody(*psi_operation);
    let psi_core::ScalarType::Integer(result_type) = result.scalar_type else {
        return Err(invalid());
    };
    let expected_result = super::unit_scalar_shape(result.value, result_type)?;
    let Some(helper) = functions
        .iter()
        .find(|function| function.machine == *callee)
    else {
        return Err(invalid());
    };
    let AssignedOperation::ReturnDynamicParameterScalarCall {
        scalar_type,
        parameter_abi,
        function_call_plan,
        ..
    } = &helper.operation
    else {
        return Err(invalid());
    };
    if !copies.is_empty()
        || dynamic_arguments.len() != 1
        || *scalar_type != result.scalar_type
        || function_call_plan != call_plan
        || result_home.defining_operation != *psi_operation
        || result_home.source_value != result.value
        || result_home.scalar_type != result_type
        || result_home.shape != expected_result
        || call_plan.result.as_ref().map(|placement| placement.shape) != Some(expected_result)
        || call_plan.parameters.len() != 2
        || parameter_abi.parameter != dynamic_arguments[0].custody.target
    {
        return Err(invalid());
    }

    let mut emitted_arguments = Vec::with_capacity(dynamic_arguments.len());
    for (ordinal, argument) in dynamic_arguments.iter().enumerate() {
        if !argument
            .custody
            .has_complete_custody(owner, *psi_operation, *callee)
        {
            return Err(invalid());
        }
        let omega_target_operations::AbstractDynamicDescriptorSource::Rebound { rebound, .. } =
            &argument.custody.source
        else {
            return Err(invalid());
        };
        if argument.instance.place != rebound.source.place
            || argument.instance.access != rebound.source.access
            || argument.instance.path != rebound.source.path
        {
            return Err(invalid());
        }
        let instance_index = ordinal.checked_mul(2).ok_or_else(invalid)?;
        let instance_placement = call_plan
            .parameters
            .get(instance_index)
            .ok_or_else(invalid)?;
        let table_placement = call_plan
            .parameters
            .get(instance_index + 1)
            .ok_or_else(invalid)?;
        if exact_register(instance_placement) != Some(argument.instance.destination)
            || exact_register(table_placement) != Some(argument.table_destination)
        {
            return Err(invalid());
        }
        let (source_home_byte_offset, source_home_indirect, instance_offset, instance_count) =
            match target.architecture {
                Architecture::X86_64 => {
                    emit_x86_instance(bytes, argument, x86_homes, *psi_operation)?
                }
                Architecture::Aarch64 => {
                    emit_aarch64_instance(bytes, argument, aarch64_homes, *psi_operation)?
                }
            };
        let table_address = match target.architecture {
            Architecture::X86_64 => emit_x86_table_address(bytes, argument.table_destination)?,
            Architecture::Aarch64 => emit_aarch64_table_address(bytes, argument.table_destination)?,
        };
        let adapters = build_adapters(argument, target, functions, *psi_operation)?;
        emitted_arguments.push(ForwardedDynamicDescriptorArgumentRecord {
            custody: argument.custody.clone(),
            instance: target_instance(argument, instance_placement.clone()),
            instance_destination: argument.instance.destination,
            table_destination: argument.table_destination,
            source_home_byte_offset,
            source_home_indirect,
            instance_code_offset: instance_offset,
            instance_byte_count: instance_count,
            table_address,
            adapters,
        });
    }

    let relocation_index = internal_calls.len();
    match target.architecture {
        Architecture::X86_64 => {
            emit_x86_64_unit_call(
                bytes,
                CallSiteOwner::Operation(*psi_operation),
                *callee,
                &[],
                target,
                x86_homes,
                &[],
                internal_calls,
            )?;
        }
        Architecture::Aarch64 => {
            emit_aarch64_unit_call(
                bytes,
                CallSiteOwner::Operation(*psi_operation),
                *callee,
                &[],
                aarch64_homes,
                &[],
                internal_calls,
            )?;
        }
    }
    let relocation = internal_calls.get(relocation_index).ok_or_else(invalid)?;
    if internal_calls.len() != relocation_index + 1
        || relocation.owner != CallSiteOwner::Operation(*psi_operation)
        || relocation.target != *callee
        || relocation.scalar_stack.is_some()
    {
        return Err(invalid());
    }
    let unit_stack = relocation.unit_stack.clone().ok_or_else(invalid)?;
    let (direct_call_offset, direct_call_byte_count) = match target.architecture {
        Architecture::X86_64 => (relocation.offset.checked_sub(1).ok_or_else(invalid)?, 5),
        Architecture::Aarch64 => (relocation.offset, 4),
    };
    let result_record = emit_unit_scalar_result(
        bytes,
        target.architecture,
        *psi_operation,
        call_plan,
        *result_home,
    )?;
    Ok(ForwardedDynamicDescriptorCallRecord {
        psi_operation: *psi_operation,
        semantic_result: *result,
        result: result_record,
        callee: *callee,
        call_plan: call_plan.clone(),
        dynamic_arguments: emitted_arguments,
        claim_transfers: claim_transfers.clone(),
        direct_call_offset,
        direct_call_byte_count,
        unit_stack,
        operation_ordinal,
        code_offset,
        byte_count: bytes.len() - code_offset,
    })
}

fn build_adapters(
    argument: &AssignedDynamicDescriptorArgument,
    target: NativeTarget,
    functions: &[AssignedFunction],
    operation: psi_core::OperationId,
) -> Result<Vec<ForwardedDynamicDescriptorAdapterRecord>, EmissionError> {
    let invalid = || EmissionError::InvalidDynamicDescriptorCallCustody(operation);
    let omega_target_operations::AbstractDynamicDescriptorSource::Rebound { application, .. } =
        &argument.custody.source
    else {
        return Err(invalid());
    };
    let pointer_size = u16::try_from(target.pointer_size).map_err(|_| invalid())?;
    let pointer_alignment = u16::try_from(target.pointer_alignment).map_err(|_| invalid())?;
    let pointer_shape = ValueShape::integer(pointer_size, pointer_alignment);
    application
        .rows
        .iter()
        .enumerate()
        .map(|(row_index, row)| {
            let callable_identity = row
                .realization_callable_identity
                .as_deref()
                .ok_or_else(invalid)?;
            let callable = application
                .realization_callables
                .iter()
                .find(|candidate| candidate.source_callable_identity == callable_identity)
                .ok_or_else(invalid)?;
            let realization = functions
                .iter()
                .filter(|function| function.machine == callable.machine)
                .collect::<Vec<_>>();
            let [realization] = realization.as_slice() else {
                return Err(invalid());
            };
            if !assigned_result_matches(&realization.operation, callable.result) {
                return Err(invalid());
            }
            let result_shape = callable_result_shape(callable.result);
            let erased_call_plan = evaluate_call_plan(
                CallingPolicy::native_for_target(target),
                &CallSignature {
                    parameters: vec![pointer_shape],
                    result: result_shape,
                },
            )
            .map_err(|_| invalid())?;
            let realization_parameter_shape =
                if argument.instance.access == psi_terminal::StructuralAccess::MutableBorrow {
                    ValueShape::borrowed_reference(
                        argument.instance.shape.byte_size,
                        argument.instance.shape.alignment,
                    )
                } else {
                    argument.instance.shape
                };
            let expected_realization_call_plan = evaluate_call_plan(
                CallingPolicy::native_for_target(target),
                &CallSignature {
                    parameters: vec![realization_parameter_shape],
                    result: result_shape,
                },
            )
            .map_err(|_| invalid())?;
            let realization_abi = realization
                .mixed_structural_scalar_abi
                .as_ref()
                .ok_or_else(invalid)?;
            if !realization_abi.scalar_parameters.is_empty()
                || realization_abi.structural_parameters.len() != 1
                || realization_abi.structural_parameters[0].structural_type
                    != argument.instance.structural_type
                || realization_abi.structural_parameters[0].access != argument.instance.access
                || realization_abi.structural_parameters[0].shape != realization_parameter_shape
                || realization_abi.call_plan != expected_realization_call_plan
                || realization_abi.result.placement
                    != *expected_realization_call_plan
                        .result
                        .as_ref()
                        .ok_or_else(invalid)?
            {
                return Err(invalid());
            }
            let realization_call_plan = realization_abi.call_plan.clone();
            let identity = ForwardedDynamicDescriptorAdapterIdentity {
                application: application.commitment,
                row_index: u32::try_from(row_index).map_err(|_| invalid())?,
                realization: callable.machine,
            };
            emit_adapter(
                identity,
                row,
                callable.result,
                argument.instance.shape,
                erased_call_plan,
                realization_call_plan,
                target,
            )
        })
        .collect()
}

fn callable_result_shape(
    result: psi_terminal::ClosedConformanceCallableResult,
) -> Option<ValueShape> {
    match result {
        psi_terminal::ClosedConformanceCallableResult::Unit => None,
        psi_terminal::ClosedConformanceCallableResult::I32 => Some(ValueShape::integer(4, 4)),
        psi_terminal::ClosedConformanceCallableResult::Bool => Some(ValueShape::integer(1, 1)),
    }
}

fn assigned_result_matches(
    operation: &AssignedOperation,
    result: psi_terminal::ClosedConformanceCallableResult,
) -> bool {
    match (operation, result) {
        (
            AssignedOperation::ReturnIntegerImmediate { scalar_type, .. }
            | AssignedOperation::ReturnIntegerParameter { scalar_type, .. }
            | AssignedOperation::ReturnIntegerExpression { scalar_type, .. },
            psi_terminal::ClosedConformanceCallableResult::I32,
        ) => scalar_type.sign() == psi_core::IntegerSign::Signed && scalar_type.bits() == 32,
        (
            AssignedOperation::ReturnBooleanImmediate { .. }
            | AssignedOperation::ReturnBooleanParameter { .. }
            | AssignedOperation::ReturnBooleanNotParameter { .. }
            | AssignedOperation::ReturnBooleanSharedConvergence { .. }
            | AssignedOperation::ReturnBooleanExpression { .. },
            psi_terminal::ClosedConformanceCallableResult::Bool,
        ) => true,
        (AssignedOperation::ScalarReturnAfterStructuralScalarFieldStore { scalar, .. }, result) => {
            assigned_result_matches(scalar, result)
        }
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_adapter(
    identity: ForwardedDynamicDescriptorAdapterIdentity,
    row: &psi_terminal::ClosedConformanceRow,
    result: psi_terminal::ClosedConformanceCallableResult,
    source_shape: ValueShape,
    erased_call_plan: omega_calling_conventions::CallPlan,
    realization_call_plan: omega_calling_conventions::CallPlan,
    target: NativeTarget,
) -> Result<ForwardedDynamicDescriptorAdapterRecord, EmissionError> {
    let invalid = || EmissionError::UnitCallStackAreaNotEncodable;
    let (bytes, argument_code_offset, argument_byte_count, direct_call_offset, return_offset) =
        match target.architecture {
            Architecture::X86_64 => {
                emit_x86_adapter(&erased_call_plan, &realization_call_plan, source_shape)?
            }
            Architecture::Aarch64 => {
                emit_aarch64_adapter(&erased_call_plan, &realization_call_plan, source_shape)?
            }
        };
    let direct_call_byte_count = match target.architecture {
        Architecture::X86_64 => 5,
        Architecture::Aarch64 => 4,
    };
    let return_byte_count = match target.architecture {
        Architecture::X86_64 => 1,
        Architecture::Aarch64 => 4,
    };
    if direct_call_offset
        .checked_add(direct_call_byte_count)
        .is_none_or(|end| end > bytes.len())
        || return_offset
            .checked_add(return_byte_count)
            .is_none_or(|end| end != bytes.len())
    {
        return Err(invalid());
    }
    Ok(ForwardedDynamicDescriptorAdapterRecord {
        identity,
        requirement_identity: row.requirement_identity.clone(),
        realization_identity: row.realization_identity.clone(),
        realization_callable_identity: row
            .realization_callable_identity
            .clone()
            .ok_or_else(invalid)?,
        result,
        erased_call_plan,
        realization_call_plan,
        source_shape,
        bytes,
        argument_code_offset,
        argument_byte_count,
        direct_call_offset,
        direct_call_byte_count,
        return_offset,
        return_byte_count,
    })
}

fn emit_x86_adapter(
    erased: &omega_calling_conventions::CallPlan,
    realization: &omega_calling_conventions::CallPlan,
    source_shape: ValueShape,
) -> Result<(Vec<u8>, usize, usize, usize, usize), EmissionError> {
    let [erased_parameter] = erased.parameters.as_slice() else {
        return Err(EmissionError::UnitCallStackAreaNotEncodable);
    };
    let [realization_parameter] = realization.parameters.as_slice() else {
        return Err(EmissionError::UnitCallStackAreaNotEncodable);
    };
    let erased_register = exact_register(erased_parameter)
        .map(x86_unit_register)
        .transpose()?
        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
    let mut bytes = Vec::new();
    let argument_code_offset = bytes.len();
    materialize_x86_adapter_argument(
        &mut bytes,
        erased_register,
        realization_parameter,
        source_shape,
    )?;
    let argument_byte_count = bytes.len() - argument_code_offset;
    let shadow_bytes = u32::from(realization.shadow_bytes);
    let padding = (8 + 16 - (shadow_bytes % 16)) % 16;
    let call_stack_bytes = shadow_bytes
        .checked_add(padding)
        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
    if call_stack_bytes != 0 {
        emit_x86_64_adjust_sp(&mut bytes, call_stack_bytes, false);
    }
    let direct_call_offset = bytes.len();
    bytes.push(0xe8);
    bytes.extend_from_slice(&0_i32.to_le_bytes());
    if call_stack_bytes != 0 {
        emit_x86_64_adjust_sp(&mut bytes, call_stack_bytes, true);
    }
    let return_offset = bytes.len();
    bytes.push(0xc3);
    Ok((
        bytes,
        argument_code_offset,
        argument_byte_count,
        direct_call_offset,
        return_offset,
    ))
}

fn materialize_x86_adapter_argument(
    bytes: &mut Vec<u8>,
    erased_register: u8,
    placement: &ValuePlacement,
    source_shape: ValueShape,
) -> Result<(), EmissionError> {
    match placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] if *byte_size == source_shape.byte_size && *byte_size <= 8 => {
            emit_x86_64_memory_load_width(
                bytes,
                x86_unit_register(*register)?,
                erased_register,
                0,
                *byte_size,
            )
        }
        [
            ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(register),
                copy_stack_byte_offset: None,
                ..
            },
        ] => emit_x86_register_move(bytes, x86_unit_register(*register)?, erased_register),
        _ => Err(EmissionError::UnitCallStackAreaNotEncodable),
    }
}

fn emit_x86_register_move(
    bytes: &mut Vec<u8>,
    destination: u8,
    source: u8,
) -> Result<(), EmissionError> {
    if destination == source {
        return Ok(());
    }
    bytes.push(0x48 | (((source >> 3) & 1) << 2) | ((destination >> 3) & 1));
    bytes.push(0x89);
    bytes.push(0xc0 | ((source & 7) << 3) | (destination & 7));
    Ok(())
}

fn emit_aarch64_adapter(
    erased: &omega_calling_conventions::CallPlan,
    realization: &omega_calling_conventions::CallPlan,
    source_shape: ValueShape,
) -> Result<(Vec<u8>, usize, usize, usize, usize), EmissionError> {
    let [erased_parameter] = erased.parameters.as_slice() else {
        return Err(EmissionError::UnitCallStackAreaNotEncodable);
    };
    let [realization_parameter] = realization.parameters.as_slice() else {
        return Err(EmissionError::UnitCallStackAreaNotEncodable);
    };
    let erased_register = exact_register(erased_parameter)
        .map(aarch64_unit_register)
        .transpose()?
        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
    let mut instructions = Vec::new();
    emit_aarch64_adjust_sp(&mut instructions, 16, false)?;
    instructions.push(aarch64_unit_stack_access(aarch64_store_base(8)?, 30, 0, 8)?);
    let argument_instruction_offset = instructions.len();
    materialize_aarch64_adapter_argument(
        &mut instructions,
        erased_register,
        realization_parameter,
        source_shape,
    )?;
    let argument_instruction_count = instructions.len() - argument_instruction_offset;
    let direct_call_instruction = instructions.len();
    instructions.push(0x9400_0000);
    instructions.push(aarch64_unit_stack_access(aarch64_load_base(8)?, 30, 0, 8)?);
    emit_aarch64_adjust_sp(&mut instructions, 16, true)?;
    let return_instruction = instructions.len();
    instructions.push(0xd65f_03c0);
    let bytes = instructions
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    Ok((
        bytes,
        argument_instruction_offset * 4,
        argument_instruction_count * 4,
        direct_call_instruction * 4,
        return_instruction * 4,
    ))
}

fn materialize_aarch64_adapter_argument(
    instructions: &mut Vec<u32>,
    erased_register: u8,
    placement: &ValuePlacement,
    source_shape: ValueShape,
) -> Result<(), EmissionError> {
    match placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] if *byte_size == source_shape.byte_size && matches!(*byte_size, 1 | 2 | 4 | 8) => {
            let destination = aarch64_unit_register(*register)?;
            instructions.push(aarch64_unit_memory_access(
                aarch64_load_base(*byte_size)?,
                destination,
                erased_register,
                0,
                *byte_size,
            )?);
            Ok(())
        }
        [
            ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(register),
                copy_stack_byte_offset: None,
                ..
            },
        ] => {
            let destination = aarch64_unit_register(*register)?;
            if destination != erased_register {
                instructions.push(
                    0xaa00_03e0 | (u32::from(erased_register) << 16) | u32::from(destination),
                );
            }
            Ok(())
        }
        _ => Err(EmissionError::UnitCallStackAreaNotEncodable),
    }
}

fn exact_register(placement: &ValuePlacement) -> Option<omega_target_operations::MachineRegister> {
    let [
        omega_calling_conventions::ValueLocation::Register {
            register,
            value_byte_offset: 0,
            byte_size: _,
        },
    ] = placement.locations.as_slice()
    else {
        return None;
    };
    Some(*register)
}

fn target_instance(
    argument: &AssignedDynamicDescriptorArgument,
    destination: ValuePlacement,
) -> TargetDynamicDescriptorInstanceArgument {
    TargetDynamicDescriptorInstanceArgument {
        place: argument.instance.place,
        access: argument.instance.access,
        path: argument.instance.path.clone(),
        root_structural_type: argument.instance.root_structural_type,
        structural_type: argument.instance.structural_type,
        shape: argument.instance.shape,
        source_byte_offset: argument.instance.source_byte_offset,
        source: argument.instance.source.clone(),
        destination,
    }
}

fn emit_x86_instance(
    bytes: &mut Vec<u8>,
    argument: &AssignedDynamicDescriptorArgument,
    homes: &[X86UnitParameterHome],
    operation: psi_core::OperationId,
) -> Result<(u32, bool, usize, usize), EmissionError> {
    let home = homes
        .iter()
        .find(|home| home.place == argument.instance.place)
        .ok_or(EmissionError::MissingUnitParameterHome(
            argument.instance.place,
        ))?;
    if home.source != argument.instance.source
        || argument
            .instance
            .source_byte_offset
            .checked_add(u32::from(argument.instance.shape.byte_size))
            .is_none_or(|end| end > u32::from(home.shape.byte_size))
    {
        return Err(EmissionError::InvalidDynamicDescriptorCallCustody(
            operation,
        ));
    }
    let register = x86_unit_register(argument.instance.destination)?;
    let offset = bytes.len();
    if home.indirect {
        emit_x86_64_stack_load_width(bytes, register, home.byte_offset, 8)?;
        emit_x86_add_immediate(bytes, register, argument.instance.source_byte_offset);
    } else {
        let source = home
            .byte_offset
            .checked_add(argument.instance.source_byte_offset)
            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
        super::dynamic_scalar::emit_x86_64_stack_address(bytes, register, source)?;
    }
    Ok((
        home.byte_offset,
        home.indirect,
        offset,
        bytes.len() - offset,
    ))
}

fn emit_x86_add_immediate(bytes: &mut Vec<u8>, register: u8, immediate: u32) {
    if immediate == 0 {
        return;
    }
    bytes.extend_from_slice(&[0x48 | ((register >> 3) & 1), 0x81, 0xc0 | (register & 7)]);
    bytes.extend_from_slice(&immediate.to_le_bytes());
}

fn emit_x86_table_address(
    bytes: &mut Vec<u8>,
    destination: omega_target_operations::MachineRegister,
) -> Result<DynamicTableAddressMaterialization, EmissionError> {
    let register = x86_unit_register(destination)?;
    let code_offset = bytes.len();
    bytes.push(0x48 | (((register >> 3) & 1) << 2));
    bytes.push(0x8d);
    bytes.push(0x05 | ((register & 7) << 3));
    let relocation_offset = bytes.len();
    bytes.extend_from_slice(&0_i32.to_le_bytes());
    Ok(DynamicTableAddressMaterialization {
        code_offset,
        byte_count: bytes.len() - code_offset,
        encoding: DynamicTableAddressEncoding::X86_64Relative32 { relocation_offset },
    })
}

fn emit_aarch64_instance(
    bytes: &mut Vec<u8>,
    argument: &AssignedDynamicDescriptorArgument,
    homes: &[Aarch64UnitParameterHome],
    operation: psi_core::OperationId,
) -> Result<(u32, bool, usize, usize), EmissionError> {
    let home = homes
        .iter()
        .find(|home| home.place == argument.instance.place)
        .ok_or(EmissionError::MissingUnitParameterHome(
            argument.instance.place,
        ))?;
    if home.source != argument.instance.source
        || argument.instance.source_byte_offset > 0xfff
        || argument
            .instance
            .source_byte_offset
            .checked_add(u32::from(argument.instance.shape.byte_size))
            .is_none_or(|end| end > u32::from(home.shape.byte_size))
    {
        return Err(EmissionError::InvalidDynamicDescriptorCallCustody(
            operation,
        ));
    }
    let register = aarch64_unit_register(argument.instance.destination)?;
    let code_offset = bytes.len();
    let mut instructions = Vec::new();
    if home.indirect {
        instructions.push(aarch64_unit_stack_access(
            aarch64_load_base(8)?,
            register,
            home.byte_offset,
            8,
        )?);
        if argument.instance.source_byte_offset != 0 {
            instructions.push(
                0x9100_0000
                    | (argument.instance.source_byte_offset << 10)
                    | (u32::from(register) << 5)
                    | u32::from(register),
            );
        }
    } else {
        emit_aarch64_sp_address(
            &mut instructions,
            register,
            home.byte_offset
                .checked_add(argument.instance.source_byte_offset)
                .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?,
        )?;
    }
    append_aarch64_instructions(bytes, instructions);
    Ok((
        home.byte_offset,
        home.indirect,
        code_offset,
        bytes.len() - code_offset,
    ))
}

fn emit_aarch64_table_address(
    bytes: &mut Vec<u8>,
    destination: omega_target_operations::MachineRegister,
) -> Result<DynamicTableAddressMaterialization, EmissionError> {
    let register = aarch64_unit_register(destination)?;
    let code_offset = bytes.len();
    let page_relocation_offset = bytes.len();
    bytes.extend_from_slice(&(0x9000_0000 | u32::from(register)).to_le_bytes());
    let page_offset_relocation_offset = bytes.len();
    bytes.extend_from_slice(
        &(0x9100_0000 | (u32::from(register) << 5) | u32::from(register)).to_le_bytes(),
    );
    Ok(DynamicTableAddressMaterialization {
        code_offset,
        byte_count: bytes.len() - code_offset,
        encoding: DynamicTableAddressEncoding::Aarch64PageAddress {
            page_relocation_offset,
            page_offset_relocation_offset,
        },
    })
}
