//! Caller-side materialization for forwarded existential descriptors.

use omega_assigned_target_operations::{
    AssignedDynamicDescriptorArgument, AssignedFunction, AssignedOperation, AssignedUnitOperation,
};
use omega_calling_conventions::ValuePlacement;
use omega_machine_code::{
    DynamicTableAddressEncoding, DynamicTableAddressMaterialization,
    ForwardedDynamicDescriptorArgumentRecord, ForwardedDynamicDescriptorCallRecord,
    InternalCallRelocation,
};
use omega_target::{Architecture, NativeTarget};
use omega_target_operations::{CallSiteOwner, TargetDynamicDescriptorInstanceArgument};

use super::{
    emit_aarch64_unit_call, emit_x86_64_unit_call, Aarch64UnitParameterHome, X86UnitParameterHome,
};
use crate::{
    aarch64_load_base, aarch64_unit_register, aarch64_unit_stack_access,
    append_aarch64_instructions, emit_aarch64_sp_address, emit_x86_64_stack_load_width,
    x86_unit_register, EmissionError,
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
    Ok(ForwardedDynamicDescriptorCallRecord {
        psi_operation: *psi_operation,
        result: *result,
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

fn exact_register(placement: &ValuePlacement) -> Option<omega_target_operations::MachineRegister> {
    let [omega_calling_conventions::ValueLocation::Register {
        register,
        value_byte_offset: 0,
        byte_size: _,
    }] = placement.locations.as_slice()
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
