//! Bounded structural-scalar store and call emission for attached Unit bodies.

use std::collections::BTreeMap;

use omega_assigned_target_operations::{
    AssignedFunction, AssignedOperation, AssignedUnitBody, AssignedUnitOperation,
    AssignedUnitScalarArgumentSource,
};
use omega_machine_code::{
    InternalCallRelocation, InternalUnitCallArgumentRecord, InternalUnitCallRecord,
    InternalUnitScalarArgumentSourceRecord, UnitStructuralScalarFieldStoreRecord,
};
use omega_target::{Architecture, NativeTarget};
use omega_target_operations::CallSiteOwner;
use psi_core::{IntegerType, IntegerValue, OperationId, ValueId};

use super::{
    Aarch64UnitParameterHome, X86UnitParameterHome, emit_aarch64_unit_call, emit_x86_64_unit_call,
    unit_scalar_shape,
};
use crate::{
    EmissionError, aarch64_load_base, aarch64_store_base, aarch64_unit_memory_access,
    aarch64_unit_stack_access, append_aarch64_instructions, emit_x86_64_stack_load_width,
    emit_x86_64_stack_store_width, integer_bits, require_native_integer_width,
};

pub(super) fn emit_structural_scalar_field_store(
    operation: &AssignedUnitOperation,
    body: &AssignedUnitBody,
    attachment: Option<psi_core::StructuralTypeId>,
    target: NativeTarget,
    x86_homes: &[X86UnitParameterHome],
    aarch64_homes: &[Aarch64UnitParameterHome],
    established_integer_constants: &BTreeMap<ValueId, (OperationId, IntegerType, IntegerValue)>,
    bytes: &mut Vec<u8>,
    operation_ordinal: usize,
    code_offset: usize,
) -> Result<UnitStructuralScalarFieldStoreRecord, EmissionError> {
    let AssignedUnitOperation::StructuralScalarFieldStore {
        psi_operation,
        destination,
        path,
        field,
        destination_placement,
        field_byte_offset,
        source,
    } = operation
    else {
        unreachable!("structural-scalar store router supplied another operation")
    };
    let AssignedUnitScalarArgumentSource::IntegerImmediate {
        defining_operation,
        source_value,
        scalar_type,
        value,
    } = *source
    else {
        return Err(EmissionError::InvalidStructuralScalarFieldStoreCustody(
            *psi_operation,
        ));
    };
    if established_integer_constants.get(&source_value)
        != Some(&(defining_operation, scalar_type, value))
        || !destination.is_self
        || attachment != Some(destination.structural_type)
    {
        return Err(EmissionError::InvalidStructuralScalarFieldStoreCustody(
            *psi_operation,
        ));
    }
    let parameter_index = usize::try_from(destination.position)
        .map_err(|_| EmissionError::InvalidStructuralScalarFieldStoreCustody(*psi_operation))?;
    let parameter = body.parameters.get(parameter_index).ok_or(
        EmissionError::InvalidStructuralScalarFieldStoreCustody(*psi_operation),
    )?;
    if parameter.place != destination.place
        || parameter.structural_type != destination.structural_type
        || parameter.multiplicity != destination.multiplicity
        || parameter.access != destination.access
        || parameter.projected_qualifications != destination.projected_qualifications
        || &parameter.placement != destination_placement
        || path.is_empty()
    {
        return Err(EmissionError::InvalidStructuralScalarFieldStoreCustody(
            *psi_operation,
        ));
    }
    let width = require_native_integer_width(source_value, scalar_type)? / 8;
    if field_byte_offset
        .checked_add(u32::from(width))
        .is_none_or(|end| end > u32::from(parameter.shape.byte_size))
    {
        return Err(EmissionError::InvalidStructuralScalarFieldStoreCustody(
            *psi_operation,
        ));
    }
    let bits = integer_bits(source_value, scalar_type, value)?;
    let (parameter_home_byte_offset, parameter_home_indirect) = match target.architecture {
        Architecture::X86_64 => {
            let home = x86_homes
                .iter()
                .find(|home| home.place == destination.place)
                .ok_or(EmissionError::MissingUnitParameterHome(destination.place))?;
            if home.source != *destination_placement || home.shape != parameter.shape {
                return Err(EmissionError::UnitParameterHomeMismatch(destination.place));
            }
            emit_x86_64_unit_store_immediate(bytes, home, *field_byte_offset, width, bits)?;
            (home.byte_offset, home.indirect)
        }
        Architecture::Aarch64 => {
            let home = aarch64_homes
                .iter()
                .find(|home| home.place == destination.place)
                .ok_or(EmissionError::MissingUnitParameterHome(destination.place))?;
            if home.source != *destination_placement || home.shape != parameter.shape {
                return Err(EmissionError::UnitParameterHomeMismatch(destination.place));
            }
            emit_aarch64_unit_store_immediate(bytes, home, *field_byte_offset, width, bits)?;
            (home.byte_offset, home.indirect)
        }
    };
    Ok(UnitStructuralScalarFieldStoreRecord {
        psi_operation: *psi_operation,
        destination: destination.clone(),
        path: path.clone(),
        field: *field,
        destination_placement: destination_placement.clone(),
        field_byte_offset: *field_byte_offset,
        source: InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            defining_operation,
            source_value,
            scalar_type,
            value,
        },
        parameter_home_byte_offset,
        parameter_home_indirect,
        operation_ordinal,
        code_offset,
        byte_count: bytes.len() - code_offset,
        bytes: bytes[code_offset..].to_vec(),
    })
}

pub(super) fn emit_structural_scalar_call(
    operation: &AssignedUnitOperation,
    target: NativeTarget,
    functions: &[AssignedFunction],
    x86_homes: &[X86UnitParameterHome],
    aarch64_homes: &[Aarch64UnitParameterHome],
    bytes: &mut Vec<u8>,
    internal_calls: &mut Vec<InternalCallRelocation>,
    operation_ordinal: usize,
    code_offset: usize,
) -> Result<InternalUnitCallRecord, EmissionError> {
    let AssignedUnitOperation::StructuralScalarCall {
        psi_operation,
        result,
        callee,
        call_plan,
        copies,
        claim_transfers,
        ..
    } = operation
    else {
        unreachable!("structural-scalar call router supplied another operation")
    };
    let psi_core::ScalarType::Integer(integer_type) = result.scalar_type else {
        return Err(EmissionError::InvalidStructuralScalarCallCustody(
            *psi_operation,
        ));
    };
    let result_shape = unit_scalar_shape(result.value, integer_type)?;
    if call_plan.result.as_ref().map(|placement| placement.shape) != Some(result_shape)
        || call_plan.parameters.len() != copies.len()
        || call_plan
            .parameters
            .iter()
            .zip(copies)
            .any(|(placement, copy)| placement != &copy.destination)
        || !functions.iter().any(|function| {
            function.machine == *callee
                && assigned_integer_result_matches(&function.operation, integer_type)
        })
    {
        return Err(EmissionError::InvalidStructuralScalarCallCustody(
            *psi_operation,
        ));
    }
    let argument_intervals = match target.architecture {
        Architecture::X86_64 => emit_x86_64_unit_call(
            bytes,
            CallSiteOwner::Operation(*psi_operation),
            *callee,
            copies,
            target,
            x86_homes,
            &[],
            internal_calls,
        )?,
        Architecture::Aarch64 => emit_aarch64_unit_call(
            bytes,
            CallSiteOwner::Operation(*psi_operation),
            *callee,
            copies,
            aarch64_homes,
            &[],
            internal_calls,
        )?,
    };
    Ok(InternalUnitCallRecord {
        owner: CallSiteOwner::Operation(*psi_operation),
        target: *callee,
        result: Some(result.scalar_type),
        semantic_result: Some(result.clone()),
        structural_result: None,
        arguments: copies
            .iter()
            .zip(argument_intervals)
            .map(
                |(copy, (code_offset, byte_count, source_home_byte_offset, call_stack_bytes))| {
                    InternalUnitCallArgumentRecord {
                        place: copy.place,
                        access: copy.access,
                        path: copy.path.clone(),
                        root_structural_type: copy.root_structural_type,
                        structural_type: copy.structural_type,
                        shape: copy.shape,
                        source_byte_offset: copy.source_byte_offset,
                        source_home_byte_offset,
                        call_stack_bytes,
                        fixed_array_length: copy.fixed_array_length,
                        element_stride: copy.element_stride,
                        source: copy.source.clone(),
                        destination: copy.destination.clone(),
                        code_offset,
                        byte_count,
                        bytes: bytes[code_offset..code_offset + byte_count].to_vec(),
                    }
                },
            )
            .collect(),
        claim_transfers: claim_transfers.clone(),
        operation_ordinal,
        code_offset,
        byte_count: bytes.len() - code_offset,
    })
}

fn assigned_integer_result_matches(
    operation: &AssignedOperation,
    expected: psi_core::IntegerType,
) -> bool {
    match operation {
        AssignedOperation::ReturnIntegerImmediate { scalar_type, .. }
        | AssignedOperation::ReturnIntegerParameter { scalar_type, .. }
        | AssignedOperation::ReturnIntegerExpression { scalar_type, .. } => {
            *scalar_type == expected
        }
        AssignedOperation::ScalarReturnWithCleanup { scalar, .. } => {
            assigned_integer_result_matches(scalar, expected)
        }
        _ => false,
    }
}

fn emit_x86_64_unit_store_immediate(
    bytes: &mut Vec<u8>,
    home: &X86UnitParameterHome,
    field_byte_offset: u32,
    byte_size: u16,
    bits: u64,
) -> Result<(), EmissionError> {
    const ADDRESS_REGISTER: u8 = 10;
    const VALUE_REGISTER: u8 = 11;
    bytes.push(0x49);
    bytes.push(0xb8 | (VALUE_REGISTER & 7));
    bytes.extend_from_slice(&bits.to_le_bytes());
    if home.indirect {
        emit_x86_64_stack_load_width(bytes, ADDRESS_REGISTER, home.byte_offset, 8)?;
        emit_x86_64_memory_store_width(
            bytes,
            VALUE_REGISTER,
            ADDRESS_REGISTER,
            field_byte_offset,
            byte_size,
        )
    } else {
        let destination = home
            .byte_offset
            .checked_add(field_byte_offset)
            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
        emit_x86_64_stack_store_width(bytes, VALUE_REGISTER, destination, byte_size)
    }
}

fn emit_x86_64_memory_store_width(
    bytes: &mut Vec<u8>,
    source: u8,
    base: u8,
    byte_offset: u32,
    byte_size: u16,
) -> Result<(), EmissionError> {
    match byte_size {
        1 => bytes.push(0x40 | (((source >> 3) & 1) << 2) | ((base >> 3) & 1)),
        2 => {
            bytes.push(0x66);
            bytes.push(0x40 | (((source >> 3) & 1) << 2) | ((base >> 3) & 1));
        }
        4 => bytes.push(0x40 | (((source >> 3) & 1) << 2) | ((base >> 3) & 1)),
        8 => bytes.push(0x48 | (((source >> 3) & 1) << 2) | ((base >> 3) & 1)),
        width => return Err(EmissionError::UnsupportedAggregateFragmentWidth(width)),
    }
    bytes.push(if byte_size == 1 { 0x88 } else { 0x89 });
    if byte_offset == 0 && (base & 7) != 5 {
        bytes.push(((source & 7) << 3) | (base & 7));
    } else if byte_offset <= i8::MAX as u32 {
        bytes.push(0x40 | ((source & 7) << 3) | (base & 7));
        bytes.push(byte_offset as u8);
    } else {
        bytes.push(0x80 | ((source & 7) << 3) | (base & 7));
        bytes.extend_from_slice(&byte_offset.to_le_bytes());
    }
    Ok(())
}

fn emit_aarch64_unit_store_immediate(
    bytes: &mut Vec<u8>,
    home: &Aarch64UnitParameterHome,
    field_byte_offset: u32,
    byte_size: u16,
    bits: u64,
) -> Result<(), EmissionError> {
    const ADDRESS_REGISTER: u8 = 17;
    const VALUE_REGISTER: u8 = 16;
    let mut instructions = Vec::new();
    emit_aarch64_unit_immediate(&mut instructions, VALUE_REGISTER, bits);
    if home.indirect {
        instructions.push(aarch64_unit_stack_access(
            aarch64_load_base(8)?,
            ADDRESS_REGISTER,
            home.byte_offset,
            8,
        )?);
        instructions.push(aarch64_unit_memory_access(
            aarch64_store_base(byte_size)?,
            VALUE_REGISTER,
            ADDRESS_REGISTER,
            field_byte_offset,
            byte_size,
        )?);
    } else {
        let destination = home
            .byte_offset
            .checked_add(field_byte_offset)
            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
        instructions.push(aarch64_unit_stack_access(
            aarch64_store_base(byte_size)?,
            VALUE_REGISTER,
            destination,
            byte_size,
        )?);
    }
    append_aarch64_instructions(bytes, instructions);
    Ok(())
}

fn emit_aarch64_unit_immediate(instructions: &mut Vec<u32>, register: u8, bits: u64) {
    for chunk in 0..4 {
        let immediate = ((bits >> (chunk * 16)) & 0xffff) as u32;
        if chunk == 0 || immediate != 0 {
            let base = if chunk == 0 { 0xd280_0000 } else { 0xf280_0000 };
            instructions
                .push(base | ((chunk as u32) << 21) | (immediate << 5) | u32::from(register));
        }
    }
}
