//! Bounded structural-scalar store and call emission for attached Unit bodies.

use std::collections::BTreeMap;

use omega_assigned_target_operations::{
    AssignedFunction, AssignedOperation, AssignedUnitBody, AssignedUnitOperation,
    AssignedUnitScalarArgumentSource,
};
use omega_calling_conventions::{CallSignature, CallingPolicy, evaluate_call_plan};
use omega_machine_code::{
    InternalCallRelocation, InternalUnitCallArgumentRecord, InternalUnitCallRecord,
    InternalUnitScalarArgumentSourceRecord, InternalUnitScalarCallArgumentRecord,
    UnitCallStackEvidence, UnitStructuralScalarFieldStoreRecord,
};
use omega_target::{Architecture, NativeTarget};
use omega_target_operations::CallSiteOwner;
use psi_core::{IntegerType, IntegerValue, OperationId, ValueId};

use super::{
    Aarch64UnitParameterHome, X86UnitParameterHome, aarch64_outgoing_placement_extent, align_u32,
    emit_aarch64_adjust_sp, emit_aarch64_aggregate_copy_from_home, emit_x86_64_adjust_sp,
    emit_x86_64_aggregate_copy_from_home, outgoing_placement_extent, stack_adjustment_pair,
    unit_scalar_shape,
};
use crate::{
    EmissionError, aarch64_load_base, aarch64_store_base, aarch64_unit_memory_access,
    aarch64_unit_stack_access, append_aarch64_instructions, emit_x86_64_stack_load_width,
    emit_x86_64_stack_store_width, integer_bits, require_native_integer_width,
};

use super::scalar_call::{
    emit_aarch64_unit_scalar_argument, emit_x86_64_unit_scalar_argument,
    unit_scalar_argument_source_record, validate_unit_scalar_argument,
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
    preceding_operations: &[AssignedUnitOperation],
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
        scalar_arguments,
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
    let invalid = || EmissionError::InvalidStructuralScalarCallCustody(*psi_operation);
    let result_shape = unit_scalar_shape(result.value, integer_type).map_err(|_| invalid())?;
    let scalar_shapes = scalar_arguments
        .iter()
        .map(|argument| {
            unit_scalar_shape(
                argument.source.source_value(),
                argument.source.scalar_type(),
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| invalid())?;
    let expected_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: scalar_shapes
                .iter()
                .copied()
                .chain(copies.iter().map(|copy| copy.shape))
                .collect(),
            result: Some(result_shape),
        },
    )
    .map_err(|_| invalid())?;
    let scalar_count = scalar_arguments.len();
    let matching_callees = functions
        .iter()
        .filter(|function| function.machine == *callee)
        .collect::<Vec<_>>();
    let [callee_function] = matching_callees.as_slice() else {
        return Err(invalid());
    };
    let mixed_abi_matches = if scalar_arguments.is_empty() {
        true
    } else {
        callee_function
            .mixed_structural_scalar_abi
            .as_ref()
            .is_some_and(|abi| {
                exact_mixed_callee_abi_matches(
                    abi,
                    call_plan,
                    result.scalar_type,
                    scalar_arguments,
                    copies,
                )
            })
    };
    if expected_call_plan != *call_plan
        || call_plan.result.as_ref().map(|placement| placement.shape) != Some(result_shape)
        || call_plan.parameters.len() != scalar_count + copies.len()
        || call_plan.parameters[..scalar_count]
            .iter()
            .zip(scalar_arguments)
            .zip(&scalar_shapes)
            .any(|((placement, argument), expected_shape)| {
                placement.shape != *expected_shape
                    || assigned_scalar_destination(placement) != Some(argument.destination)
            })
        || call_plan.parameters[scalar_count..]
            .iter()
            .zip(copies)
            .any(|(placement, copy)| placement != &copy.destination)
        || callee_function.fixed_integer_scalar_abi.is_some()
        || !assigned_integer_result_matches(&callee_function.operation, integer_type)
        || !mixed_abi_matches
        || !explicit_callee_call_plan_matches(&callee_function.operation, call_plan, copies)
    {
        return Err(invalid());
    }
    let (scalar_argument_records, argument_intervals) = match target.architecture {
        Architecture::X86_64 => emit_x86_64_mixed_call(
            bytes,
            *psi_operation,
            *callee,
            call_plan,
            scalar_arguments,
            copies,
            preceding_operations,
            x86_homes,
            internal_calls,
        )?,
        Architecture::Aarch64 => emit_aarch64_mixed_call(
            bytes,
            *psi_operation,
            *callee,
            call_plan,
            scalar_arguments,
            copies,
            preceding_operations,
            aarch64_homes,
            internal_calls,
        )?,
    };
    Ok(InternalUnitCallRecord {
        owner: CallSiteOwner::Operation(*psi_operation),
        target: *callee,
        result: Some(result.scalar_type),
        semantic_result: Some(result.clone()),
        structural_result: None,
        scalar_arguments: scalar_argument_records,
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

fn exact_mixed_callee_abi_matches(
    abi: &omega_target_operations::MixedStructuralScalarFunctionAbi,
    call_plan: &omega_calling_conventions::CallPlan,
    result_type: psi_core::ScalarType,
    scalar_arguments: &[omega_assigned_target_operations::AssignedUnitScalarCallArgument],
    copies: &[omega_assigned_target_operations::AssignedAggregateCopy],
) -> bool {
    &abi.call_plan == call_plan
        && abi.result.scalar_type
            == match result_type {
                psi_core::ScalarType::Integer(integer_type) => integer_type,
                _ => return false,
            }
        && call_plan.result.as_ref() == Some(&abi.result.placement)
        && abi.scalar_parameters.len() == scalar_arguments.len()
        && abi
            .scalar_parameters
            .iter()
            .zip(scalar_arguments)
            .all(|(parameter, argument)| {
                parameter.scalar_type == argument.source.scalar_type()
                    && usize::try_from(argument.parameter_index)
                        .ok()
                        .and_then(|index| call_plan.parameters.get(index))
                        == Some(&parameter.placement)
                    && assigned_scalar_destination(&parameter.placement)
                        == Some(argument.destination)
            })
        && abi.structural_parameters.len() == copies.len()
        && abi
            .structural_parameters
            .iter()
            .zip(copies)
            .all(|(parameter, copy)| {
                parameter.structural_type == copy.structural_type
                    && parameter.shape == copy.shape
                    && parameter.access == copy.access
                    && parameter.placement == copy.destination
            })
}

fn assigned_scalar_destination(
    placement: &omega_calling_conventions::ValuePlacement,
) -> Option<omega_assigned_target_operations::AssignedCallDestination> {
    match placement.locations.as_slice() {
        [
            omega_calling_conventions::ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] if *byte_size == placement.shape.byte_size => {
            Some(omega_assigned_target_operations::AssignedCallDestination::Register(*register))
        }
        [
            omega_calling_conventions::ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset: 0,
                byte_size,
                ..
            },
        ] if *byte_size == placement.shape.byte_size => Some(
            omega_assigned_target_operations::AssignedCallDestination::OutgoingStack {
                byte_offset: *stack_byte_offset,
            },
        ),
        _ => None,
    }
}

fn explicit_callee_call_plan_matches(
    operation: &AssignedOperation,
    call_plan: &omega_calling_conventions::CallPlan,
    copies: &[omega_assigned_target_operations::AssignedAggregateCopy],
) -> bool {
    match operation {
        AssignedOperation::ReturnIntegerImmediate { .. }
        | AssignedOperation::ReturnIntegerParameter { .. }
        | AssignedOperation::ReturnIntegerExpression { .. } => true,
        AssignedOperation::ScalarReturnWithCleanup {
            call_plan: callee_call_plan,
            structural_parameters,
            ..
        } => {
            callee_call_plan == call_plan
                && structural_parameters.len() == copies.len()
                && structural_parameters
                    .iter()
                    .zip(copies)
                    .all(|(parameter, copy)| {
                        parameter.structural_type == copy.structural_type
                            && parameter.shape == copy.shape
                            && parameter.placement == copy.destination
                    })
        }
        _ => false,
    }
}

type AggregateArgumentInterval = (usize, usize, u32, u32);

#[allow(clippy::too_many_arguments)]
fn emit_x86_64_mixed_call(
    bytes: &mut Vec<u8>,
    psi_operation: OperationId,
    callee: psi_core::MachineId,
    call_plan: &omega_calling_conventions::CallPlan,
    scalar_arguments: &[omega_assigned_target_operations::AssignedUnitScalarCallArgument],
    copies: &[omega_assigned_target_operations::AssignedAggregateCopy],
    preceding_operations: &[AssignedUnitOperation],
    homes: &[X86UnitParameterHome],
    internal_calls: &mut Vec<InternalCallRelocation>,
) -> Result<
    (
        Vec<InternalUnitScalarCallArgumentRecord>,
        Vec<AggregateArgumentInterval>,
    ),
    EmissionError,
> {
    for (parameter_index, argument) in scalar_arguments.iter().enumerate() {
        validate_unit_scalar_argument(
            psi_operation,
            parameter_index,
            argument,
            call_plan,
            preceding_operations,
        )
        .map_err(|_| EmissionError::InvalidStructuralScalarCallCustody(psi_operation))?;
    }
    let outgoing_bytes = call_plan
        .parameters
        .iter()
        .map(outgoing_placement_extent)
        .try_fold(u32::from(call_plan.shadow_bytes), |extent, candidate| {
            candidate.map(|value| extent.max(value))
        })?;
    let padding = (8 + 16 - (outgoing_bytes % 16)) % 16;
    let call_stack_bytes = outgoing_bytes
        .checked_add(padding)
        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
    let mut allocation = None;
    if call_stack_bytes != 0 {
        let offset = bytes.len();
        emit_x86_64_adjust_sp(bytes, call_stack_bytes, false);
        allocation = Some((offset, bytes.len() - offset));
    }

    let mut scalar_records = Vec::with_capacity(scalar_arguments.len());
    for (parameter_index, argument) in scalar_arguments.iter().enumerate() {
        let code_offset = bytes.len();
        emit_x86_64_unit_scalar_argument(bytes, argument, call_stack_bytes)?;
        scalar_records.push(InternalUnitScalarCallArgumentRecord {
            parameter_index: argument.parameter_index,
            source: unit_scalar_argument_source_record(argument.source),
            destination: call_plan.parameters[parameter_index].clone(),
            code_offset,
            byte_count: bytes.len() - code_offset,
        });
    }

    let mut aggregate_intervals = Vec::with_capacity(copies.len());
    for copy in copies {
        let code_offset = bytes.len();
        let home = homes
            .iter()
            .find(|home| home.place == copy.place)
            .ok_or(EmissionError::MissingUnitParameterHome(copy.place))?;
        if home.source != copy.source
            || copy
                .source_byte_offset
                .checked_add(u32::from(copy.shape.byte_size))
                .is_none_or(|end| end > u32::from(home.shape.byte_size))
        {
            return Err(EmissionError::UnitParameterHomeMismatch(copy.place));
        }
        emit_x86_64_aggregate_copy_from_home(bytes, copy, home, call_stack_bytes)?;
        aggregate_intervals.push((
            code_offset,
            bytes.len() - code_offset,
            home.byte_offset,
            call_stack_bytes,
        ));
    }

    bytes.push(0xe8);
    let relocation_offset = bytes.len();
    bytes.extend_from_slice(&0_i32.to_le_bytes());
    let mut release = None;
    if call_stack_bytes != 0 {
        let offset = bytes.len();
        emit_x86_64_adjust_sp(bytes, call_stack_bytes, true);
        release = Some((offset, bytes.len() - offset));
    }
    internal_calls.push(InternalCallRelocation {
        owner: CallSiteOwner::Operation(psi_operation),
        target: callee,
        unit_stack: Some(UnitCallStackEvidence {
            outbound: stack_adjustment_pair(call_stack_bytes, allocation, release),
        }),
        scalar_stack: None,
        offset: relocation_offset,
    });
    Ok((scalar_records, aggregate_intervals))
}

#[allow(clippy::too_many_arguments)]
fn emit_aarch64_mixed_call(
    bytes: &mut Vec<u8>,
    psi_operation: OperationId,
    callee: psi_core::MachineId,
    call_plan: &omega_calling_conventions::CallPlan,
    scalar_arguments: &[omega_assigned_target_operations::AssignedUnitScalarCallArgument],
    copies: &[omega_assigned_target_operations::AssignedAggregateCopy],
    preceding_operations: &[AssignedUnitOperation],
    homes: &[Aarch64UnitParameterHome],
    internal_calls: &mut Vec<InternalCallRelocation>,
) -> Result<
    (
        Vec<InternalUnitScalarCallArgumentRecord>,
        Vec<AggregateArgumentInterval>,
    ),
    EmissionError,
> {
    for (parameter_index, argument) in scalar_arguments.iter().enumerate() {
        validate_unit_scalar_argument(
            psi_operation,
            parameter_index,
            argument,
            call_plan,
            preceding_operations,
        )
        .map_err(|_| EmissionError::InvalidStructuralScalarCallCustody(psi_operation))?;
    }
    let outgoing_bytes = call_plan
        .parameters
        .iter()
        .map(aarch64_outgoing_placement_extent)
        .try_fold(u32::from(call_plan.shadow_bytes), |extent, candidate| {
            candidate.map(|value| extent.max(value))
        })?;
    let call_stack_bytes = align_u32(outgoing_bytes, 16)?;
    let mut allocation = None;
    if call_stack_bytes != 0 {
        let mut instructions = Vec::new();
        emit_aarch64_adjust_sp(&mut instructions, call_stack_bytes, false)?;
        let offset = bytes.len();
        append_aarch64_instructions(bytes, instructions);
        allocation = Some((offset, bytes.len() - offset));
    }

    let mut scalar_records = Vec::with_capacity(scalar_arguments.len());
    for (parameter_index, argument) in scalar_arguments.iter().enumerate() {
        let code_offset = bytes.len();
        emit_aarch64_unit_scalar_argument(bytes, argument, call_stack_bytes)?;
        scalar_records.push(InternalUnitScalarCallArgumentRecord {
            parameter_index: argument.parameter_index,
            source: unit_scalar_argument_source_record(argument.source),
            destination: call_plan.parameters[parameter_index].clone(),
            code_offset,
            byte_count: bytes.len() - code_offset,
        });
    }

    let mut aggregate_intervals = Vec::with_capacity(copies.len());
    for copy in copies {
        let home = homes
            .iter()
            .find(|home| home.place == copy.place)
            .ok_or(EmissionError::MissingUnitParameterHome(copy.place))?;
        if home.source != copy.source
            || copy
                .source_byte_offset
                .checked_add(u32::from(copy.shape.byte_size))
                .is_none_or(|end| end > u32::from(home.shape.byte_size))
        {
            return Err(EmissionError::UnitParameterHomeMismatch(copy.place));
        }
        let mut instructions = Vec::new();
        emit_aarch64_aggregate_copy_from_home(&mut instructions, copy, home, call_stack_bytes)?;
        let code_offset = bytes.len();
        append_aarch64_instructions(bytes, instructions);
        aggregate_intervals.push((
            code_offset,
            bytes.len() - code_offset,
            home.byte_offset,
            call_stack_bytes,
        ));
    }

    let relocation_offset = bytes.len();
    bytes.extend_from_slice(&0x9400_0000_u32.to_le_bytes());
    let mut release = None;
    if call_stack_bytes != 0 {
        let mut instructions = Vec::new();
        emit_aarch64_adjust_sp(&mut instructions, call_stack_bytes, true)?;
        let offset = bytes.len();
        append_aarch64_instructions(bytes, instructions);
        release = Some((offset, bytes.len() - offset));
    }
    internal_calls.push(InternalCallRelocation {
        owner: CallSiteOwner::Operation(psi_operation),
        target: callee,
        unit_stack: Some(UnitCallStackEvidence {
            outbound: stack_adjustment_pair(call_stack_bytes, allocation, release),
        }),
        scalar_stack: None,
        offset: relocation_offset,
    });
    Ok((scalar_records, aggregate_intervals))
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

pub(crate) fn emit_x86_64_memory_store_width(
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

pub(crate) fn emit_aarch64_unit_immediate(instructions: &mut Vec<u32>, register: u8, bits: u64) {
    for chunk in 0..4 {
        let immediate = ((bits >> (chunk * 16)) & 0xffff) as u32;
        if chunk == 0 || immediate != 0 {
            let base = if chunk == 0 { 0xd280_0000 } else { 0xf280_0000 };
            instructions
                .push(base | ((chunk as u32) << 21) | (immediate << 5) | u32::from(register));
        }
    }
}
