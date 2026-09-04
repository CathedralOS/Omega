//! Bounded structural-scalar store and call emission for attached Unit bodies.

use std::collections::BTreeMap;

use omega_assigned_target_operations::{
    AssignedFunction, AssignedOperation, AssignedScalarLocation, AssignedUnitBody,
    AssignedUnitOperation, AssignedUnitScalarArgumentSource,
};
use omega_calling_conventions::{CallSignature, CallingPolicy, ValueLocation, evaluate_call_plan};
use omega_machine_code::{
    InternalCallRelocation, InternalStructuralCallResult, InternalUnitCallArgumentRecord,
    InternalUnitCallRecord, InternalUnitScalarArgumentSourceRecord,
    InternalUnitScalarCallArgumentRecord, UnitCallStackEvidence,
    UnitStructuralScalarFieldStoreRecord,
};
use omega_target::{Architecture, NativeTarget};
use omega_target_operations::CallSiteOwner;
use psi_core::{IntegerType, IntegerValue, OperationId, ScalarType, ValueId};

use super::{
    Aarch64UnitParameterHome, X86UnitParameterHome, emit_aarch64_adjust_sp,
    emit_aarch64_aggregate_copy_from_home, emit_x86_64_adjust_sp,
    emit_x86_64_aggregate_copy_from_home, stack_adjustment_pair, unit_scalar_home_record,
    unit_scalar_shape,
};
use crate::{
    EmissionError, aarch64_load_base, aarch64_store_base, aarch64_unit_memory_access,
    aarch64_unit_stack_access, append_aarch64_instructions, emit_x86_64_stack_load_width,
    emit_x86_64_stack_store_width, integer_bits, require_native_integer_width,
};

use super::scalar_call::{
    aarch64_unit_scalar_transport_plan, emit_aarch64_scalar_snapshots,
    emit_aarch64_unit_scalar_argument, emit_x86_64_scalar_snapshots,
    emit_x86_64_unit_scalar_argument, unit_scalar_argument_source_record,
    validate_unit_scalar_argument, x86_unit_scalar_transport_plan,
};

pub(super) fn emit_structural_scalar_field_store(
    operation: &AssignedUnitOperation,
    body: &AssignedUnitBody,
    attachment: Option<psi_core::StructuralTypeId>,
    target: NativeTarget,
    x86_homes: &[X86UnitParameterHome],
    aarch64_homes: &[Aarch64UnitParameterHome],
    x86_frame_bytes: u32,
    aarch64_frame_bytes: u32,
    established_integer_constants: &BTreeMap<ValueId, (OperationId, IntegerType, IntegerValue)>,
    established_boolean_constants: &BTreeMap<ValueId, (OperationId, bool, usize)>,
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
    let invalid = || EmissionError::InvalidStructuralScalarFieldStoreCustody(*psi_operation);
    let (source_record, width, immediate_bits) = match *source {
        AssignedUnitScalarArgumentSource::Parameter {
            parameter_index,
            source_value,
            scalar_type,
            location,
        } => {
            let scalar_parameter_index = usize::try_from(parameter_index).map_err(|_| invalid())?;
            let scalar_parameter = body
                .scalar_parameters
                .get(scalar_parameter_index)
                .ok_or_else(invalid)?;
            let width = match scalar_type {
                ScalarType::Boolean => 1,
                ScalarType::Integer(integer) => {
                    require_native_integer_width(source_value, integer)? / 8
                }
                ScalarType::IeeeFloat(_) => return Err(invalid()),
            };
            let parameter_shapes = body
                .scalar_parameters
                .iter()
                .map(|parameter| {
                    let shape = unit_scalar_shape(parameter.value, parameter.scalar_type)?;
                    (parameter.placement.shape == shape)
                        .then_some(shape)
                        .ok_or_else(invalid)
                })
                .chain(body.parameters.iter().map(|parameter| Ok(parameter.shape)))
                .collect::<Result<Vec<_>, EmissionError>>()?;
            let expected_plan = evaluate_call_plan(
                CallingPolicy::native_for_target(target),
                &CallSignature {
                    parameters: parameter_shapes,
                    result: None,
                },
            )
            .map_err(|_| invalid())?;
            let expected_location = match scalar_parameter.placement.locations.as_slice() {
                [
                    ValueLocation::Register {
                        register,
                        value_byte_offset: 0,
                        byte_size,
                    },
                ] if *byte_size == width => AssignedScalarLocation::Register(*register),
                [
                    ValueLocation::Stack {
                        stack_byte_offset,
                        value_byte_offset: 0,
                        byte_size,
                        ..
                    },
                ] if *byte_size == width => AssignedScalarLocation::IncomingStack {
                    byte_offset: *stack_byte_offset,
                },
                _ => return Err(invalid()),
            };
            if body.parameters.len() != 1
                || scalar_parameter.value != source_value
                || scalar_parameter.scalar_type != scalar_type
                || location != expected_location
                || body.call_plan != expected_plan
                || body.call_plan.parameters.get(scalar_parameter_index)
                    != Some(&scalar_parameter.placement)
                || body.call_plan.parameters.get(body.scalar_parameters.len())
                    != Some(destination_placement)
            {
                return Err(invalid());
            }
            let location = match expected_location {
                AssignedScalarLocation::Register(register) => {
                    omega_machine_code::UnitScalarParameterLocationRecord::Register(register)
                }
                AssignedScalarLocation::IncomingStack { byte_offset } => {
                    omega_machine_code::UnitScalarParameterLocationRecord::IncomingStack {
                        byte_offset,
                    }
                }
                AssignedScalarLocation::FrameSpill { .. } => return Err(invalid()),
            };
            (
                InternalUnitScalarArgumentSourceRecord::Parameter {
                    parameter_index,
                    source_value,
                    scalar_type,
                    location,
                },
                width,
                None,
            )
        }
        AssignedUnitScalarArgumentSource::IntegerImmediate {
            defining_operation,
            source_value,
            scalar_type,
            value,
        } => {
            if established_integer_constants.get(&source_value)
                != Some(&(defining_operation, scalar_type, value))
            {
                return Err(invalid());
            }
            (
                InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
                    defining_operation,
                    source_value,
                    scalar_type,
                    value,
                },
                require_native_integer_width(source_value, scalar_type)? / 8,
                Some(integer_bits(source_value, scalar_type, value)?),
            )
        }
        AssignedUnitScalarArgumentSource::BooleanImmediate {
            defining_operation,
            source_value,
            value,
        } => {
            let Some((retained_operation, retained_value, definition_ordinal)) =
                established_boolean_constants.get(&source_value).copied()
            else {
                return Err(invalid());
            };
            if retained_operation != defining_operation || retained_value != value {
                return Err(invalid());
            }
            (
                InternalUnitScalarArgumentSourceRecord::BooleanImmediate {
                    defining_operation,
                    source_value,
                    value,
                    definition_ordinal,
                },
                1,
                Some(u64::from(value)),
            )
        }
        AssignedUnitScalarArgumentSource::Home(home) => {
            let exact_source_count = body.operations[..operation_ordinal]
                .iter()
                .filter(|operation| {
                    matches!(
                        operation,
                        AssignedUnitOperation::ScalarCall { result_home, .. }
                            if *result_home == home
                    )
                })
                .count();
            let ScalarType::Integer(integer) = home.scalar_type else {
                return Err(invalid());
            };
            if exact_source_count != 1
                || home.shape != unit_scalar_shape(home.source_value, home.scalar_type)?
            {
                return Err(invalid());
            }
            (
                InternalUnitScalarArgumentSourceRecord::Home(unit_scalar_home_record(home)),
                require_native_integer_width(home.source_value, integer)? / 8,
                None,
            )
        }
    };
    if destination.is_self && attachment != Some(destination.structural_type) {
        return Err(invalid());
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
    {
        return Err(EmissionError::InvalidStructuralScalarFieldStoreCustody(
            *psi_operation,
        ));
    }
    if field_byte_offset
        .checked_add(u32::from(width))
        .is_none_or(|end| end > u32::from(parameter.shape.byte_size))
    {
        return Err(EmissionError::InvalidStructuralScalarFieldStoreCustody(
            *psi_operation,
        ));
    }
    let (parameter_home_byte_offset, parameter_home_indirect) = match target.architecture {
        Architecture::X86_64 => {
            let home = x86_homes
                .iter()
                .find(|home| home.place == destination.place)
                .ok_or(EmissionError::MissingUnitParameterHome(destination.place))?;
            if home.source != *destination_placement || home.shape != parameter.shape {
                return Err(EmissionError::UnitParameterHomeMismatch(destination.place));
            }
            match source_record {
                InternalUnitScalarArgumentSourceRecord::Parameter {
                    location:
                        omega_machine_code::UnitScalarParameterLocationRecord::Register(register),
                    ..
                } => emit_x86_64_unit_store_register(
                    bytes,
                    home,
                    *field_byte_offset,
                    width,
                    register,
                )?,
                InternalUnitScalarArgumentSourceRecord::Parameter {
                    location:
                        omega_machine_code::UnitScalarParameterLocationRecord::IncomingStack {
                            byte_offset,
                        },
                    ..
                } => {
                    let source_offset = x86_frame_bytes
                        .checked_add(8)
                        .and_then(|offset| offset.checked_add(byte_offset))
                        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                    emit_x86_64_stack_load_width(bytes, 11, source_offset, width)?;
                    emit_x86_64_unit_store_register(
                        bytes,
                        home,
                        *field_byte_offset,
                        width,
                        omega_target_operations::MachineRegister::X86R11,
                    )?;
                }
                InternalUnitScalarArgumentSourceRecord::Home(source_home) => {
                    emit_x86_64_stack_load_width(bytes, 11, source_home.byte_offset, 8)?;
                    emit_x86_64_unit_store_register(
                        bytes,
                        home,
                        *field_byte_offset,
                        width,
                        omega_target_operations::MachineRegister::X86R11,
                    )?;
                }
                _ => emit_x86_64_unit_store_immediate(
                    bytes,
                    home,
                    *field_byte_offset,
                    width,
                    immediate_bits.ok_or_else(invalid)?,
                )?,
            }
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
            match source_record {
                InternalUnitScalarArgumentSourceRecord::Parameter {
                    location:
                        omega_machine_code::UnitScalarParameterLocationRecord::Register(register),
                    ..
                } => emit_aarch64_unit_store_register(
                    bytes,
                    home,
                    *field_byte_offset,
                    width,
                    register,
                )?,
                InternalUnitScalarArgumentSourceRecord::Parameter {
                    location:
                        omega_machine_code::UnitScalarParameterLocationRecord::IncomingStack {
                            byte_offset,
                        },
                    ..
                } => {
                    let source_offset = aarch64_frame_bytes
                        .checked_add(byte_offset)
                        .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                    let instruction = aarch64_unit_stack_access(
                        aarch64_load_base(width)?,
                        16,
                        source_offset,
                        width,
                    )?;
                    append_aarch64_instructions(bytes, vec![instruction]);
                    emit_aarch64_unit_store_register(
                        bytes,
                        home,
                        *field_byte_offset,
                        width,
                        omega_target_operations::MachineRegister::Aarch64X(16),
                    )?;
                }
                InternalUnitScalarArgumentSourceRecord::Home(source_home) => {
                    let instruction = aarch64_unit_stack_access(
                        aarch64_load_base(8)?,
                        16,
                        source_home.byte_offset,
                        8,
                    )?;
                    append_aarch64_instructions(bytes, vec![instruction]);
                    emit_aarch64_unit_store_register(
                        bytes,
                        home,
                        *field_byte_offset,
                        width,
                        omega_target_operations::MachineRegister::Aarch64X(16),
                    )?;
                }
                _ => emit_aarch64_unit_store_immediate(
                    bytes,
                    home,
                    *field_byte_offset,
                    width,
                    immediate_bits.ok_or_else(invalid)?,
                )?,
            }
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
        source: source_record,
        parameter_home_byte_offset,
        parameter_home_indirect,
        operation_ordinal,
        code_offset,
        byte_count: bytes.len() - code_offset,
        bytes: bytes[code_offset..].to_vec(),
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_unit_result_call(
    operation: &AssignedUnitOperation,
    caller_scalar_parameters: &[omega_target_operations::UnitScalarAbiValue],
    target: NativeTarget,
    functions: &[AssignedFunction],
    preceding_operations: &[AssignedUnitOperation],
    x86_homes: &[X86UnitParameterHome],
    aarch64_homes: &[Aarch64UnitParameterHome],
    x86_frame_bytes: u32,
    aarch64_frame_bytes: u32,
    bytes: &mut Vec<u8>,
    internal_calls: &mut Vec<InternalCallRelocation>,
    operation_ordinal: usize,
    code_offset: usize,
) -> Result<InternalUnitCallRecord, EmissionError> {
    let AssignedUnitOperation::Call {
        psi_operation,
        callee,
        result: None,
        call_plan,
        scalar_arguments,
        copies,
        claim_transfers,
        ..
    } = operation
    else {
        return Err(EmissionError::UnsupportedAggregatePlacement);
    };
    let invalid = || EmissionError::InvalidUnitScalarCallCustody(*psi_operation);
    let scalar_shapes = scalar_arguments
        .iter()
        .map(|argument| {
            unit_scalar_shape(
                argument.source.source_value(),
                argument.source.scalar_type(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let expected_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: scalar_shapes
                .iter()
                .copied()
                .chain(copies.iter().map(|copy| copy.shape))
                .collect(),
            result: None,
        },
    )
    .map_err(|_| invalid())?;
    let matching_callees = functions
        .iter()
        .filter(|function| function.machine == *callee)
        .collect::<Vec<_>>();
    let [callee_function] = matching_callees.as_slice() else {
        return Err(invalid());
    };
    let AssignedOperation::UnitBody(callee_body) = &callee_function.operation else {
        return Err(invalid());
    };
    let scalar_count = scalar_arguments.len();
    if scalar_count == 0
        || expected_call_plan != *call_plan
        || call_plan.result.is_some()
        || callee_body.call_plan != *call_plan
        || callee_body.scalar_parameters.len() != scalar_count
        || callee_body.parameters.len() != copies.len()
        || callee_body
            .scalar_parameters
            .iter()
            .zip(scalar_arguments)
            .enumerate()
            .any(|(index, (parameter, argument))| {
                usize::try_from(argument.parameter_index) != Ok(index)
                    || parameter.scalar_type != argument.source.scalar_type()
                    || call_plan.parameters.get(index) != Some(&parameter.placement)
            })
        || callee_body.parameters.iter().zip(copies).enumerate().any(
            |(index, (parameter, copy))| {
                parameter.structural_type != copy.structural_type
                    || parameter.access != copy.access
                    || parameter.shape != copy.shape
                    || call_plan.parameters.get(scalar_count + index) != Some(&parameter.placement)
                    || parameter.placement != copy.destination
            },
        )
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
            caller_scalar_parameters,
            copies,
            preceding_operations,
            x86_homes,
            x86_frame_bytes,
            internal_calls,
        )?,
        Architecture::Aarch64 => emit_aarch64_mixed_call(
            bytes,
            *psi_operation,
            *callee,
            call_plan,
            scalar_arguments,
            caller_scalar_parameters,
            copies,
            preceding_operations,
            aarch64_homes,
            aarch64_frame_bytes,
            internal_calls,
        )?,
    };
    Ok(InternalUnitCallRecord {
        owner: CallSiteOwner::Operation(*psi_operation),
        target: *callee,
        result: None,
        semantic_result: None,
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

pub(super) fn emit_structural_scalar_call(
    operation: &AssignedUnitOperation,
    caller_scalar_parameters: &[omega_target_operations::UnitScalarAbiValue],
    target: NativeTarget,
    functions: &[AssignedFunction],
    preceding_operations: &[AssignedUnitOperation],
    x86_homes: &[X86UnitParameterHome],
    aarch64_homes: &[Aarch64UnitParameterHome],
    x86_frame_bytes: u32,
    aarch64_frame_bytes: u32,
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
    let result_shape = unit_scalar_shape(result.value, psi_core::ScalarType::Integer(integer_type))
        .map_err(|_| invalid())?;
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
            caller_scalar_parameters,
            copies,
            preceding_operations,
            x86_homes,
            x86_frame_bytes,
            internal_calls,
        )?,
        Architecture::Aarch64 => emit_aarch64_mixed_call(
            bytes,
            *psi_operation,
            *callee,
            call_plan,
            scalar_arguments,
            caller_scalar_parameters,
            copies,
            preceding_operations,
            aarch64_homes,
            aarch64_frame_bytes,
            internal_calls,
        )?,
    };
    Ok(InternalUnitCallRecord {
        owner: CallSiteOwner::Operation(*psi_operation),
        target: *callee,
        result: Some(result.scalar_type),
        semantic_result: Some(*result),
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

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_structural_result_call(
    operation: &AssignedUnitOperation,
    caller_scalar_parameters: &[omega_target_operations::UnitScalarAbiValue],
    target: NativeTarget,
    functions: &[AssignedFunction],
    preceding_operations: &[AssignedUnitOperation],
    x86_homes: &[X86UnitParameterHome],
    aarch64_homes: &[Aarch64UnitParameterHome],
    x86_frame_bytes: u32,
    aarch64_frame_bytes: u32,
    bytes: &mut Vec<u8>,
    internal_calls: &mut Vec<InternalCallRelocation>,
    operation_ordinal: usize,
    code_offset: usize,
) -> Result<InternalUnitCallRecord, EmissionError> {
    let AssignedUnitOperation::StructuralResultCall {
        psi_operation,
        result,
        callee,
        callee_result,
        call_plan,
        scalar_arguments,
        copies,
        claim_transfers,
        returned_claim_transfers,
        requirement_obligations,
        crash_continuations,
    } = operation
    else {
        unreachable!("structural-result call router supplied another operation")
    };
    let invalid = || EmissionError::InvalidStructuralScalarCallCustody(*psi_operation);
    let ([scalar_argument], [copy]) = (scalar_arguments.as_slice(), copies.as_slice()) else {
        return Err(invalid());
    };
    let scalar_shape = unit_scalar_shape(
        scalar_argument.source.source_value(),
        scalar_argument.source.scalar_type(),
    )
    .map_err(|_| invalid())?;
    let expected_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![scalar_shape, copy.shape],
            result: Some(copy.shape),
        },
    )
    .map_err(|_| invalid())?;
    let matching_callees = functions
        .iter()
        .filter(|function| function.machine == *callee)
        .collect::<Vec<_>>();
    let [callee_function] = matching_callees.as_slice() else {
        return Err(invalid());
    };
    let AssignedOperation::ReturnStructuralParameter {
        call_plan: callee_call_plan,
        scalar_parameters: callee_scalar_parameters,
        parameters: callee_parameters,
        source: callee_source,
        result: retained_callee_result,
        shape: callee_shape,
        source_placement,
        result_placement: callee_result_placement,
        returned_claims,
        trivial_affine_locals,
        trivial_affine_discards,
        ..
    } = &callee_function.operation
    else {
        return Err(invalid());
    };
    let ([callee_scalar], [callee_parameter]) = (
        callee_scalar_parameters.as_slice(),
        callee_parameters.as_slice(),
    ) else {
        return Err(invalid());
    };
    let caller_result_placement = call_plan.result.clone().ok_or_else(invalid)?;
    if expected_call_plan != *call_plan
        || callee_call_plan != call_plan
        || call_plan.parameters.as_slice()
            != [callee_scalar.placement.clone(), source_placement.clone()]
        || scalar_argument.parameter_index != 0
        || scalar_argument.source.scalar_type()
            != psi_core::ScalarType::Integer(callee_scalar.scalar_type)
        || assigned_scalar_destination(&callee_scalar.placement)
            != Some(scalar_argument.destination)
        || copy.destination != *source_placement
        || copy.structural_type != callee_parameter.structural_type
        || copy.shape != *callee_shape
        || result.structural_type != callee_result.structural_type
        || result.structural_type != retained_callee_result.structural_type
        || result.multiplicity != psi_terminal::StructuralMultiplicity::Affine
        || callee_result.multiplicity != psi_terminal::StructuralMultiplicity::Affine
        || retained_callee_result.multiplicity != psi_terminal::StructuralMultiplicity::Affine
        || callee_parameter.multiplicity != psi_terminal::StructuralMultiplicity::Affine
        || callee_parameter.access != psi_terminal::StructuralAccess::Owned
        || callee_source != callee_parameter
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
        || callee_result != retained_callee_result
        || !claim_transfers.is_empty()
        || !returned_claim_transfers.is_empty()
        || !returned_claims.is_empty()
        || !requirement_obligations.is_empty()
        || !crash_continuations.is_empty()
        || !trivial_affine_locals.is_empty()
        || !trivial_affine_discards.is_empty()
        || caller_result_placement != *callee_result_placement
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
            caller_scalar_parameters,
            copies,
            preceding_operations,
            x86_homes,
            x86_frame_bytes,
            internal_calls,
        )?,
        Architecture::Aarch64 => emit_aarch64_mixed_call(
            bytes,
            *psi_operation,
            *callee,
            call_plan,
            scalar_arguments,
            caller_scalar_parameters,
            copies,
            preceding_operations,
            aarch64_homes,
            aarch64_frame_bytes,
            internal_calls,
        )?,
    };
    let [argument_interval] = argument_intervals.as_slice() else {
        return Err(invalid());
    };
    let (argument_code_offset, argument_byte_count, source_home_byte_offset, call_stack_bytes) =
        *argument_interval;
    Ok(InternalUnitCallRecord {
        owner: CallSiteOwner::Operation(*psi_operation),
        target: *callee,
        result: None,
        semantic_result: None,
        structural_result: Some(InternalStructuralCallResult {
            operation_result: result.clone(),
            function_result: callee_result.clone(),
            returned_claim_transfers: Vec::new(),
            returned_claims: Vec::new(),
            caller_result_placement,
            callee_result_placement: callee_result_placement.clone(),
        }),
        scalar_arguments: scalar_argument_records,
        arguments: vec![InternalUnitCallArgumentRecord {
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
            code_offset: argument_code_offset,
            byte_count: argument_byte_count,
            bytes: bytes[argument_code_offset..argument_code_offset + argument_byte_count].to_vec(),
        }],
        claim_transfers: Vec::new(),
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
        && abi.result.scalar_type == result_type
        && call_plan.result.as_ref() == Some(&abi.result.placement)
        && abi.scalar_parameters.len() == scalar_arguments.len()
        && abi
            .scalar_parameters
            .iter()
            .zip(scalar_arguments)
            .all(|(parameter, argument)| {
                psi_core::ScalarType::Integer(parameter.scalar_type)
                    == argument.source.scalar_type()
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
    caller_scalar_parameters: &[omega_target_operations::UnitScalarAbiValue],
    copies: &[omega_assigned_target_operations::AssignedAggregateCopy],
    preceding_operations: &[AssignedUnitOperation],
    homes: &[X86UnitParameterHome],
    frame_bytes: u32,
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
            caller_scalar_parameters,
            preceding_operations,
        )
        .map_err(|_| EmissionError::InvalidStructuralScalarCallCustody(psi_operation))?;
    }
    let transport = x86_unit_scalar_transport_plan(call_plan, scalar_arguments, 0)?;
    let call_stack_bytes = transport.call_stack_bytes;
    let mut allocation = None;
    if call_stack_bytes != 0 {
        let offset = bytes.len();
        emit_x86_64_adjust_sp(bytes, call_stack_bytes, false);
        allocation = Some((offset, bytes.len() - offset));
    }

    let mut scalar_records = Vec::with_capacity(scalar_arguments.len());
    for (parameter_index, argument) in scalar_arguments.iter().enumerate() {
        let code_offset = bytes.len();
        if parameter_index == 0 {
            emit_x86_64_scalar_snapshots(bytes, &transport)?;
        }
        emit_x86_64_unit_scalar_argument(
            bytes,
            argument,
            frame_bytes,
            call_stack_bytes,
            &transport,
        )?;
        scalar_records.push(InternalUnitScalarCallArgumentRecord {
            parameter_index: argument.parameter_index,
            source: unit_scalar_argument_source_record(
                psi_operation,
                argument.source,
                preceding_operations,
            )?,
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
    caller_scalar_parameters: &[omega_target_operations::UnitScalarAbiValue],
    copies: &[omega_assigned_target_operations::AssignedAggregateCopy],
    preceding_operations: &[AssignedUnitOperation],
    homes: &[Aarch64UnitParameterHome],
    frame_bytes: u32,
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
            caller_scalar_parameters,
            preceding_operations,
        )
        .map_err(|_| EmissionError::InvalidStructuralScalarCallCustody(psi_operation))?;
    }
    let transport = aarch64_unit_scalar_transport_plan(call_plan, scalar_arguments)?;
    let call_stack_bytes = transport.call_stack_bytes;
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
        if parameter_index == 0 {
            emit_aarch64_scalar_snapshots(bytes, &transport)?;
        }
        emit_aarch64_unit_scalar_argument(
            bytes,
            argument,
            frame_bytes,
            call_stack_bytes,
            &transport,
        )?;
        scalar_records.push(InternalUnitScalarCallArgumentRecord {
            parameter_index: argument.parameter_index,
            source: unit_scalar_argument_source_record(
                psi_operation,
                argument.source,
                preceding_operations,
            )?,
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

pub(super) fn emit_x86_64_unit_store_immediate(
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

pub(super) fn emit_x86_64_unit_store_register(
    bytes: &mut Vec<u8>,
    home: &X86UnitParameterHome,
    field_byte_offset: u32,
    byte_size: u16,
    source: omega_target_operations::MachineRegister,
) -> Result<(), EmissionError> {
    const ADDRESS_REGISTER: u8 = 10;
    let source = crate::x86_unit_register(source)?;
    if source == ADDRESS_REGISTER {
        return Err(EmissionError::UnsupportedUnitRegister(
            omega_target_operations::MachineRegister::X86R10,
        ));
    }
    if home.indirect {
        emit_x86_64_stack_load_width(bytes, ADDRESS_REGISTER, home.byte_offset, 8)?;
        emit_x86_64_memory_store_width(
            bytes,
            source,
            ADDRESS_REGISTER,
            field_byte_offset,
            byte_size,
        )
    } else {
        let destination = home
            .byte_offset
            .checked_add(field_byte_offset)
            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
        emit_x86_64_stack_store_width(bytes, source, destination, byte_size)
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

pub(super) fn emit_aarch64_unit_store_immediate(
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

pub(super) fn emit_aarch64_unit_store_register(
    bytes: &mut Vec<u8>,
    home: &Aarch64UnitParameterHome,
    field_byte_offset: u32,
    byte_size: u16,
    source: omega_target_operations::MachineRegister,
) -> Result<(), EmissionError> {
    const ADDRESS_REGISTER: u8 = 17;
    let source = crate::aarch64_unit_register(source)?;
    if source == ADDRESS_REGISTER {
        return Err(EmissionError::UnsupportedUnitRegister(
            omega_target_operations::MachineRegister::Aarch64X(ADDRESS_REGISTER),
        ));
    }
    let mut instructions = Vec::new();
    if home.indirect {
        instructions.push(aarch64_unit_stack_access(
            aarch64_load_base(8)?,
            ADDRESS_REGISTER,
            home.byte_offset,
            8,
        )?);
        instructions.push(aarch64_unit_memory_access(
            aarch64_store_base(byte_size)?,
            source,
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
            source,
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
