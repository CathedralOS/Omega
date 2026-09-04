use omega_assigned_target_operations::{
    AssignedUnitOperation, AssignedUnitScalarArgumentSource, AssignedUnitScalarCallArgument,
    AssignedUnitScalarHome,
};
use omega_calling_conventions::{
    CallSignature, CallingPolicy, MachineRegister, ValueLocation, ValuePlacement,
    evaluate_call_plan,
};
use omega_machine_code::{
    InternalCallRelocation, InternalUnitScalarArgumentSourceRecord,
    InternalUnitScalarCallArgumentRecord, InternalUnitScalarCallRecord,
    InternalUnitScalarCallResultRecord, UnitCallStackEvidence,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use omega_target_operations::CallSiteOwner;
use psi_core::MachineId;

use super::{
    EmissionError, aarch64_load_base, aarch64_outgoing_placement_extent, aarch64_store_base,
    aarch64_unit_register, aarch64_unit_stack_access, align_u32, append_aarch64_instructions,
    emit_aarch64_adjust_sp, emit_x86_64_adjust_sp, emit_x86_64_stack_load_width,
    emit_x86_64_stack_store_width, outgoing_placement_extent, stack_adjustment_pair,
    unit_scalar_home_record, unit_scalar_shape, x86_unit_register,
};

#[derive(Debug, Clone)]
pub(super) struct UnitScalarTransportPlan {
    pub(super) call_stack_bytes: u32,
    snapshot_slots: Vec<(MachineRegister, u32)>,
}

fn scalar_snapshot_registers(arguments: &[AssignedUnitScalarCallArgument]) -> Vec<MachineRegister> {
    let source_registers = arguments
        .iter()
        .filter_map(|argument| match argument.source {
            AssignedUnitScalarArgumentSource::Parameter {
                location:
                    omega_assigned_target_operations::AssignedScalarLocation::Register(register),
                ..
            } => Some(register),
            _ => None,
        })
        .collect::<Vec<_>>();
    let needs_snapshot = arguments.iter().any(|argument| {
        let omega_assigned_target_operations::AssignedCallDestination::Register(destination) =
            argument.destination
        else {
            return false;
        };
        source_registers.contains(&destination)
            && !matches!(
                argument.source,
                AssignedUnitScalarArgumentSource::Parameter {
                    location:
                        omega_assigned_target_operations::AssignedScalarLocation::Register(source),
                    ..
                } if source == destination
            )
    });
    if !needs_snapshot {
        return Vec::new();
    }
    source_registers
        .into_iter()
        .fold(Vec::new(), |mut registers, register| {
            if !registers.contains(&register) {
                registers.push(register);
            }
            registers
        })
}

fn scalar_transport_extent(
    outgoing_bytes: u32,
    arguments: &[AssignedUnitScalarCallArgument],
) -> Result<(u32, Vec<(MachineRegister, u32)>), EmissionError> {
    let registers = scalar_snapshot_registers(arguments);
    if registers.is_empty() {
        return Ok((outgoing_bytes, Vec::new()));
    }
    let mut cursor = align_u32(outgoing_bytes, 8)?;
    let mut slots = Vec::with_capacity(registers.len());
    for register in registers {
        slots.push((register, cursor));
        cursor = cursor
            .checked_add(8)
            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
    }
    Ok((cursor, slots))
}

pub(super) fn x86_unit_scalar_transport_plan(
    call_plan: &omega_calling_conventions::CallPlan,
    arguments: &[AssignedUnitScalarCallArgument],
    minimum_outgoing_bytes: u32,
) -> Result<UnitScalarTransportPlan, EmissionError> {
    let outgoing_bytes = call_plan
        .parameters
        .iter()
        .map(outgoing_placement_extent)
        .try_fold(
            u32::from(call_plan.shadow_bytes).max(minimum_outgoing_bytes),
            |extent, candidate| candidate.map(|value| extent.max(value)),
        )?;
    let (transport_bytes, snapshot_slots) = scalar_transport_extent(outgoing_bytes, arguments)?;
    let padding = (8 + 16 - (transport_bytes % 16)) % 16;
    Ok(UnitScalarTransportPlan {
        call_stack_bytes: transport_bytes
            .checked_add(padding)
            .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?,
        snapshot_slots,
    })
}

pub(super) fn aarch64_unit_scalar_transport_plan(
    call_plan: &omega_calling_conventions::CallPlan,
    arguments: &[AssignedUnitScalarCallArgument],
) -> Result<UnitScalarTransportPlan, EmissionError> {
    let outgoing_bytes = call_plan
        .parameters
        .iter()
        .map(aarch64_outgoing_placement_extent)
        .try_fold(u32::from(call_plan.shadow_bytes), |extent, candidate| {
            candidate.map(|value| extent.max(value))
        })?;
    let (transport_bytes, snapshot_slots) = scalar_transport_extent(outgoing_bytes, arguments)?;
    Ok(UnitScalarTransportPlan {
        call_stack_bytes: align_u32(transport_bytes, 16)?,
        snapshot_slots,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_unit_scalar_call(
    bytes: &mut Vec<u8>,
    target: NativeTarget,
    psi_operation: psi_core::OperationId,
    callee: MachineId,
    call_plan: &omega_calling_conventions::CallPlan,
    result_home: AssignedUnitScalarHome,
    arguments: &[AssignedUnitScalarCallArgument],
    caller_parameters: &[omega_target_operations::UnitScalarAbiValue],
    frame_bytes: u32,
    preceding_operations: &[AssignedUnitOperation],
    operation_ordinal: usize,
    internal_calls: &mut Vec<InternalCallRelocation>,
) -> Result<InternalUnitScalarCallRecord, EmissionError> {
    let result_shape = unit_scalar_shape(result_home.source_value, result_home.scalar_type)?;
    let expected_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: arguments
                .iter()
                .map(|argument| {
                    unit_scalar_shape(
                        argument.source.source_value(),
                        argument.source.scalar_type(),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
            result: Some(result_shape),
        },
    )
    .map_err(|_| EmissionError::InvalidUnitScalarCallCustody(psi_operation))?;
    if expected_plan != *call_plan
        || result_home.defining_operation != psi_operation
        || result_home.shape != result_shape
        || call_plan.parameters.len() != arguments.len()
    {
        return Err(EmissionError::InvalidUnitScalarCallCustody(psi_operation));
    }
    let call_code_offset = bytes.len();
    let (argument_records, result_record) = match target.architecture {
        Architecture::X86_64 => emit_x86_64_unit_scalar_call(
            bytes,
            target,
            psi_operation,
            callee,
            call_plan,
            result_home,
            arguments,
            caller_parameters,
            frame_bytes,
            preceding_operations,
            internal_calls,
        )?,
        Architecture::Aarch64 => emit_aarch64_unit_scalar_call(
            bytes,
            psi_operation,
            callee,
            call_plan,
            result_home,
            arguments,
            caller_parameters,
            frame_bytes,
            preceding_operations,
            internal_calls,
        )?,
    };
    Ok(InternalUnitScalarCallRecord {
        owner: CallSiteOwner::Operation(psi_operation),
        target: callee,
        call_plan: call_plan.clone(),
        result: result_record,
        arguments: argument_records,
        operation_ordinal,
        code_offset: call_code_offset,
        byte_count: bytes.len() - call_code_offset,
    })
}

#[allow(clippy::too_many_arguments)]
fn emit_x86_64_unit_scalar_call(
    bytes: &mut Vec<u8>,
    target: NativeTarget,
    psi_operation: psi_core::OperationId,
    callee: MachineId,
    call_plan: &omega_calling_conventions::CallPlan,
    result_home: AssignedUnitScalarHome,
    arguments: &[AssignedUnitScalarCallArgument],
    caller_parameters: &[omega_target_operations::UnitScalarAbiValue],
    frame_bytes: u32,
    preceding_operations: &[AssignedUnitOperation],
    internal_calls: &mut Vec<InternalCallRelocation>,
) -> Result<
    (
        Vec<InternalUnitScalarCallArgumentRecord>,
        InternalUnitScalarCallResultRecord,
    ),
    EmissionError,
> {
    let transport = x86_unit_scalar_transport_plan(
        call_plan,
        arguments,
        if target.object_format == ObjectFormat::Coff {
            32
        } else {
            0
        },
    )?;
    let call_stack_bytes = transport.call_stack_bytes;
    let mut allocation = None;
    if call_stack_bytes != 0 {
        let offset = bytes.len();
        emit_x86_64_adjust_sp(bytes, call_stack_bytes, false);
        allocation = Some((offset, bytes.len() - offset));
    }
    let mut argument_records = Vec::with_capacity(arguments.len());
    for (parameter_index, argument) in arguments.iter().enumerate() {
        validate_unit_scalar_argument(
            psi_operation,
            parameter_index,
            argument,
            call_plan,
            caller_parameters,
            preceding_operations,
        )?;
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
        argument_records.push(InternalUnitScalarCallArgumentRecord {
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
    bytes.push(0xe8);
    let relocation_offset = bytes.len();
    bytes.extend_from_slice(&0_i32.to_le_bytes());
    let mut release = None;
    if call_stack_bytes != 0 {
        let offset = bytes.len();
        emit_x86_64_adjust_sp(bytes, call_stack_bytes, true);
        release = Some((offset, bytes.len() - offset));
    }
    let result_record = emit_unit_scalar_result(
        bytes,
        Architecture::X86_64,
        psi_operation,
        call_plan,
        result_home,
    )?;
    internal_calls.push(InternalCallRelocation {
        owner: CallSiteOwner::Operation(psi_operation),
        target: callee,
        unit_stack: Some(UnitCallStackEvidence {
            outbound: stack_adjustment_pair(call_stack_bytes, allocation, release),
        }),
        scalar_stack: None,
        offset: relocation_offset,
    });
    Ok((argument_records, result_record))
}

#[allow(clippy::too_many_arguments)]
fn emit_aarch64_unit_scalar_call(
    bytes: &mut Vec<u8>,
    psi_operation: psi_core::OperationId,
    callee: MachineId,
    call_plan: &omega_calling_conventions::CallPlan,
    result_home: AssignedUnitScalarHome,
    arguments: &[AssignedUnitScalarCallArgument],
    caller_parameters: &[omega_target_operations::UnitScalarAbiValue],
    frame_bytes: u32,
    preceding_operations: &[AssignedUnitOperation],
    internal_calls: &mut Vec<InternalCallRelocation>,
) -> Result<
    (
        Vec<InternalUnitScalarCallArgumentRecord>,
        InternalUnitScalarCallResultRecord,
    ),
    EmissionError,
> {
    let transport = aarch64_unit_scalar_transport_plan(call_plan, arguments)?;
    let call_stack_bytes = transport.call_stack_bytes;
    let mut allocation = None;
    if call_stack_bytes != 0 {
        let mut instructions = Vec::new();
        allocation = Some((bytes.len(), 4));
        emit_aarch64_adjust_sp(&mut instructions, call_stack_bytes, false)?;
        append_aarch64_instructions(bytes, instructions);
    }
    let mut argument_records = Vec::with_capacity(arguments.len());
    for (parameter_index, argument) in arguments.iter().enumerate() {
        validate_unit_scalar_argument(
            psi_operation,
            parameter_index,
            argument,
            call_plan,
            caller_parameters,
            preceding_operations,
        )?;
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
        argument_records.push(InternalUnitScalarCallArgumentRecord {
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
    let relocation_offset = bytes.len();
    bytes.extend_from_slice(&0x9400_0000_u32.to_le_bytes());
    let mut release = None;
    if call_stack_bytes != 0 {
        let mut instructions = Vec::new();
        release = Some((bytes.len(), 4));
        emit_aarch64_adjust_sp(&mut instructions, call_stack_bytes, true)?;
        append_aarch64_instructions(bytes, instructions);
    }
    let result_record = emit_unit_scalar_result(
        bytes,
        Architecture::Aarch64,
        psi_operation,
        call_plan,
        result_home,
    )?;
    internal_calls.push(InternalCallRelocation {
        owner: CallSiteOwner::Operation(psi_operation),
        target: callee,
        unit_stack: Some(UnitCallStackEvidence {
            outbound: stack_adjustment_pair(call_stack_bytes, allocation, release),
        }),
        scalar_stack: None,
        offset: relocation_offset,
    });
    Ok((argument_records, result_record))
}

pub(super) fn validate_unit_scalar_argument(
    operation: psi_core::OperationId,
    parameter_index: usize,
    argument: &AssignedUnitScalarCallArgument,
    call_plan: &omega_calling_conventions::CallPlan,
    caller_parameters: &[omega_target_operations::UnitScalarAbiValue],
    preceding_operations: &[AssignedUnitOperation],
) -> Result<(), EmissionError> {
    let Some(placement) = call_plan.parameters.get(parameter_index) else {
        return Err(EmissionError::InvalidUnitScalarCallCustody(operation));
    };
    if usize::try_from(argument.parameter_index) != Ok(parameter_index)
        || placement.shape
            != unit_scalar_shape(
                argument.source.source_value(),
                argument.source.scalar_type(),
            )?
        || assigned_destination_for_placement(placement) != Some(argument.destination)
    {
        return Err(EmissionError::InvalidUnitScalarCallCustody(operation));
    }
    if let AssignedUnitScalarArgumentSource::Parameter {
        parameter_index,
        source_value,
        scalar_type,
        location,
    } = argument.source
    {
        let index = usize::try_from(parameter_index)
            .map_err(|_| EmissionError::InvalidUnitScalarCallCustody(operation))?;
        let Some(parameter) = caller_parameters.get(index) else {
            return Err(EmissionError::InvalidUnitScalarCallCustody(operation));
        };
        let expected_location = match parameter.placement.locations.as_slice() {
            [
                ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size,
                },
            ] if *byte_size == parameter.placement.shape.byte_size => {
                omega_assigned_target_operations::AssignedScalarLocation::Register(*register)
            }
            [
                ValueLocation::Stack {
                    stack_byte_offset,
                    value_byte_offset: 0,
                    byte_size,
                    ..
                },
            ] if *byte_size == parameter.placement.shape.byte_size => {
                omega_assigned_target_operations::AssignedScalarLocation::IncomingStack {
                    byte_offset: *stack_byte_offset,
                }
            }
            _ => return Err(EmissionError::InvalidUnitScalarCallCustody(operation)),
        };
        if parameter.value != source_value
            || parameter.scalar_type != scalar_type
            || parameter.placement.shape != unit_scalar_shape(source_value, scalar_type)?
            || location != expected_location
        {
            return Err(EmissionError::InvalidUnitScalarCallCustody(operation));
        }
        return Ok(());
    }
    let exact_source_count = preceding_operations
        .iter()
        .filter(|preceding| match (preceding, argument.source) {
            (
                AssignedUnitOperation::IntegerConstant {
                    psi_operation,
                    result,
                    scalar_type,
                    value,
                },
                AssignedUnitScalarArgumentSource::IntegerImmediate {
                    defining_operation,
                    source_value,
                    scalar_type: source_type,
                    value: source_value_literal,
                },
            ) => {
                *psi_operation == defining_operation
                    && *result == source_value
                    && *scalar_type == source_type
                    && *value == source_value_literal
            }
            (
                AssignedUnitOperation::BooleanConstant {
                    psi_operation,
                    result,
                    value,
                },
                AssignedUnitScalarArgumentSource::BooleanImmediate {
                    defining_operation,
                    source_value,
                    value: source_value_literal,
                },
            ) => {
                *psi_operation == defining_operation
                    && *result == source_value
                    && *value == source_value_literal
            }
            (
                AssignedUnitOperation::ScalarCall { result_home, .. }
                | AssignedUnitOperation::DynamicScalarCall { result_home, .. }
                | AssignedUnitOperation::StoredDynamicScalarCall { result_home, .. },
                AssignedUnitScalarArgumentSource::Home(source),
            ) => *result_home == source,
            (
                AssignedUnitOperation::NormalizedForeignCall {
                    result_home: Some(result_home),
                    ..
                },
                AssignedUnitScalarArgumentSource::Home(source),
            ) => *result_home == source,
            _ => false,
        })
        .count();
    if exact_source_count != 1 {
        return Err(EmissionError::InvalidUnitScalarCallCustody(operation));
    }
    Ok(())
}

fn assigned_destination_for_placement(
    placement: &ValuePlacement,
) -> Option<omega_assigned_target_operations::AssignedCallDestination> {
    match placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] if *byte_size == placement.shape.byte_size => {
            Some(omega_assigned_target_operations::AssignedCallDestination::Register(*register))
        }
        [
            ValueLocation::Stack {
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

pub(super) fn unit_scalar_argument_source_record(
    operation: psi_core::OperationId,
    source: AssignedUnitScalarArgumentSource,
    preceding_operations: &[AssignedUnitOperation],
) -> Result<InternalUnitScalarArgumentSourceRecord, EmissionError> {
    match source {
        AssignedUnitScalarArgumentSource::Parameter {
            parameter_index,
            source_value,
            scalar_type,
            location,
        } => Ok(InternalUnitScalarArgumentSourceRecord::Parameter {
            parameter_index,
            source_value,
            scalar_type,
            location: match location {
                omega_assigned_target_operations::AssignedScalarLocation::Register(register) => {
                    omega_machine_code::UnitScalarParameterLocationRecord::Register(register)
                }
                omega_assigned_target_operations::AssignedScalarLocation::IncomingStack {
                    byte_offset,
                } => omega_machine_code::UnitScalarParameterLocationRecord::IncomingStack {
                    byte_offset,
                },
                omega_assigned_target_operations::AssignedScalarLocation::FrameSpill { .. } => {
                    unreachable!()
                }
            },
        }),
        AssignedUnitScalarArgumentSource::IntegerImmediate {
            defining_operation,
            source_value,
            scalar_type,
            value,
        } => Ok(InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            defining_operation,
            source_value,
            scalar_type,
            value,
        }),
        AssignedUnitScalarArgumentSource::BooleanImmediate {
            defining_operation,
            source_value,
            value,
        } => {
            let mut matches = preceding_operations
                .iter()
                .enumerate()
                .filter(|(_, operation)| {
                    matches!(
                        operation,
                        AssignedUnitOperation::BooleanConstant {
                            psi_operation,
                            result,
                            value: candidate,
                        } if *psi_operation == defining_operation
                            && *result == source_value
                            && *candidate == value
                    )
                });
            let Some((definition_ordinal, _)) = matches.next() else {
                return Err(EmissionError::InvalidUnitScalarCallCustody(operation));
            };
            if matches.next().is_some() {
                return Err(EmissionError::InvalidUnitScalarCallCustody(operation));
            }
            Ok(InternalUnitScalarArgumentSourceRecord::BooleanImmediate {
                defining_operation,
                source_value,
                value,
                definition_ordinal,
            })
        }
        AssignedUnitScalarArgumentSource::Home(home) => Ok(
            InternalUnitScalarArgumentSourceRecord::Home(unit_scalar_home_record(home)),
        ),
    }
}

fn scalar_result_register(
    operation: psi_core::OperationId,
    placement: Option<&ValuePlacement>,
    architecture: Architecture,
) -> Result<omega_calling_conventions::MachineRegister, EmissionError> {
    let Some(placement) = placement else {
        return Err(EmissionError::InvalidUnitScalarCallCustody(operation));
    };
    let [
        ValueLocation::Register {
            register,
            value_byte_offset: 0,
            byte_size,
        },
    ] = placement.locations.as_slice()
    else {
        return Err(EmissionError::InvalidUnitScalarCallCustody(operation));
    };
    if *byte_size != placement.shape.byte_size
        || match architecture {
            Architecture::X86_64 => x86_unit_register(*register).is_err(),
            Architecture::Aarch64 => aarch64_unit_register(*register).is_err(),
        }
    {
        return Err(EmissionError::InvalidUnitScalarCallCustody(operation));
    }
    Ok(*register)
}

/// Normalize one scalar ABI result into the shared 64-bit durable-home
/// representation and retain the exact emitted interval.
pub(super) fn emit_unit_scalar_result(
    bytes: &mut Vec<u8>,
    architecture: Architecture,
    operation: psi_core::OperationId,
    call_plan: &omega_calling_conventions::CallPlan,
    result_home: AssignedUnitScalarHome,
) -> Result<InternalUnitScalarCallResultRecord, EmissionError> {
    let code_offset = bytes.len();
    let register = scalar_result_register(operation, call_plan.result.as_ref(), architecture)?;
    match architecture {
        Architecture::X86_64 => {
            let register = x86_unit_register(register)?;
            emit_x86_64_unit_scalar_normalize(bytes, register, result_home.scalar_type);
            emit_x86_64_stack_store_width(bytes, register, result_home.byte_offset, 8)?;
        }
        Architecture::Aarch64 => {
            let register = aarch64_unit_register(register)?;
            let mut instructions = Vec::new();
            emit_aarch64_unit_scalar_normalize(
                &mut instructions,
                register,
                result_home.scalar_type,
            );
            append_aarch64_instructions(bytes, instructions);
            let instruction = aarch64_unit_stack_access(
                aarch64_store_base(8)?,
                register,
                result_home.byte_offset,
                8,
            )?;
            bytes.extend_from_slice(&instruction.to_le_bytes());
        }
    }
    Ok(InternalUnitScalarCallResultRecord {
        home: unit_scalar_home_record(result_home),
        source: call_plan.result.clone().expect("validated scalar result"),
        code_offset,
        byte_count: bytes.len() - code_offset,
    })
}

fn snapshot_offset(transport: &UnitScalarTransportPlan, register: MachineRegister) -> Option<u32> {
    transport
        .snapshot_slots
        .iter()
        .find_map(|(candidate, offset)| (*candidate == register).then_some(*offset))
}

pub(super) fn emit_x86_64_scalar_snapshots(
    bytes: &mut Vec<u8>,
    transport: &UnitScalarTransportPlan,
) -> Result<(), EmissionError> {
    for (register, offset) in &transport.snapshot_slots {
        emit_x86_64_stack_store_width(bytes, x86_unit_register(*register)?, *offset, 8)?;
    }
    Ok(())
}

pub(super) fn emit_aarch64_scalar_snapshots(
    bytes: &mut Vec<u8>,
    transport: &UnitScalarTransportPlan,
) -> Result<(), EmissionError> {
    let mut instructions = Vec::with_capacity(transport.snapshot_slots.len());
    for (register, offset) in &transport.snapshot_slots {
        instructions.push(aarch64_unit_stack_access(
            aarch64_store_base(8)?,
            aarch64_unit_register(*register)?,
            *offset,
            8,
        )?);
    }
    append_aarch64_instructions(bytes, instructions);
    Ok(())
}

fn emit_x86_64_register_move(bytes: &mut Vec<u8>, source: u8, destination: u8) {
    if source == destination {
        return;
    }
    bytes.extend_from_slice(&[
        0x48 | (((source >> 3) & 1) << 2) | ((destination >> 3) & 1),
        0x89,
        0xc0 | ((source & 7) << 3) | (destination & 7),
    ]);
}

pub(super) fn emit_x86_64_unit_scalar_argument(
    bytes: &mut Vec<u8>,
    argument: &AssignedUnitScalarCallArgument,
    frame_bytes: u32,
    call_stack_bytes: u32,
    transport: &UnitScalarTransportPlan,
) -> Result<(), EmissionError> {
    let (destination_register, destination_stack) = match argument.destination {
        omega_assigned_target_operations::AssignedCallDestination::Register(register) => {
            (x86_unit_register(register)?, None)
        }
        omega_assigned_target_operations::AssignedCallDestination::OutgoingStack {
            byte_offset,
        } => (11, Some(byte_offset)),
    };
    let mut value_register = destination_register;
    match argument.source {
        AssignedUnitScalarArgumentSource::Parameter {
            source_value,
            location,
            ..
        } => match location {
            omega_assigned_target_operations::AssignedScalarLocation::Register(source) => {
                let source_register = x86_unit_register(source)?;
                if let Some(offset) = snapshot_offset(transport, source) {
                    emit_x86_64_stack_load_width(bytes, destination_register, offset, 8)?;
                } else if destination_stack.is_some() {
                    value_register = source_register;
                } else {
                    emit_x86_64_register_move(bytes, source_register, destination_register);
                }
            }
            omega_assigned_target_operations::AssignedScalarLocation::IncomingStack {
                byte_offset,
            } => {
                let source_offset = call_stack_bytes
                    .checked_add(frame_bytes)
                    .and_then(|offset| offset.checked_add(8))
                    .and_then(|offset| offset.checked_add(byte_offset))
                    .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                let byte_size =
                    unit_scalar_shape(source_value, argument.source.scalar_type())?.byte_size;
                emit_x86_64_stack_load_width(
                    bytes,
                    destination_register,
                    source_offset,
                    byte_size,
                )?;
            }
            omega_assigned_target_operations::AssignedScalarLocation::FrameSpill { .. } => {
                return Err(EmissionError::UnsupportedUnitScalarType(source_value));
            }
        },
        AssignedUnitScalarArgumentSource::IntegerImmediate {
            source_value,
            scalar_type,
            value,
            ..
        } => emit_x86_64_unit_scalar_immediate(
            bytes,
            destination_register,
            canonical_integer_bits(source_value, scalar_type, value)?,
        ),
        AssignedUnitScalarArgumentSource::BooleanImmediate { value, .. } => {
            emit_x86_64_unit_scalar_immediate(bytes, destination_register, u64::from(value));
        }
        AssignedUnitScalarArgumentSource::Home(home) => {
            let source_offset = call_stack_bytes
                .checked_add(home.byte_offset)
                .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
            emit_x86_64_stack_load_width(bytes, destination_register, source_offset, 8)?;
        }
    }
    if let Some(destination) = destination_stack {
        emit_x86_64_stack_store_width(bytes, value_register, destination, 8)?;
    }
    Ok(())
}

pub(super) fn emit_aarch64_unit_scalar_argument(
    bytes: &mut Vec<u8>,
    argument: &AssignedUnitScalarCallArgument,
    frame_bytes: u32,
    call_stack_bytes: u32,
    transport: &UnitScalarTransportPlan,
) -> Result<(), EmissionError> {
    let (destination_register, destination_stack) = match argument.destination {
        omega_assigned_target_operations::AssignedCallDestination::Register(register) => {
            (aarch64_unit_register(register)?, None)
        }
        omega_assigned_target_operations::AssignedCallDestination::OutgoingStack {
            byte_offset,
        } => (9, Some(byte_offset)),
    };
    let mut instructions = Vec::new();
    let mut value_register = destination_register;
    match argument.source {
        AssignedUnitScalarArgumentSource::Parameter {
            source_value,
            location,
            ..
        } => match location {
            omega_assigned_target_operations::AssignedScalarLocation::Register(source) => {
                let source_register = aarch64_unit_register(source)?;
                if let Some(offset) = snapshot_offset(transport, source) {
                    instructions.push(aarch64_unit_stack_access(
                        aarch64_load_base(8)?,
                        destination_register,
                        offset,
                        8,
                    )?);
                } else if destination_stack.is_some() {
                    value_register = source_register;
                } else if source_register != destination_register {
                    instructions.push(
                        0xaa00_03e0
                            | (u32::from(source_register) << 16)
                            | u32::from(destination_register),
                    );
                }
            }
            omega_assigned_target_operations::AssignedScalarLocation::IncomingStack {
                byte_offset,
            } => {
                let source_offset = call_stack_bytes
                    .checked_add(frame_bytes)
                    .and_then(|offset| offset.checked_add(byte_offset))
                    .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
                let byte_size =
                    unit_scalar_shape(source_value, argument.source.scalar_type())?.byte_size;
                instructions.push(aarch64_unit_stack_access(
                    aarch64_load_base(byte_size)?,
                    destination_register,
                    source_offset,
                    byte_size,
                )?);
            }
            omega_assigned_target_operations::AssignedScalarLocation::FrameSpill { .. } => {
                return Err(EmissionError::UnsupportedUnitScalarType(source_value));
            }
        },
        AssignedUnitScalarArgumentSource::IntegerImmediate {
            source_value,
            scalar_type,
            value,
            ..
        } => emit_aarch64_unit_scalar_immediate(
            &mut instructions,
            destination_register,
            canonical_integer_bits(source_value, scalar_type, value)?,
        ),
        AssignedUnitScalarArgumentSource::BooleanImmediate { value, .. } => {
            emit_aarch64_unit_scalar_immediate(
                &mut instructions,
                destination_register,
                u64::from(value),
            );
        }
        AssignedUnitScalarArgumentSource::Home(home) => {
            let source_offset = call_stack_bytes
                .checked_add(home.byte_offset)
                .ok_or(EmissionError::UnitCallStackAreaNotEncodable)?;
            instructions.push(aarch64_unit_stack_access(
                aarch64_load_base(8)?,
                destination_register,
                source_offset,
                8,
            )?);
        }
    }
    if let Some(destination) = destination_stack {
        instructions.push(aarch64_unit_stack_access(
            aarch64_store_base(8)?,
            value_register,
            destination,
            8,
        )?);
    }
    append_aarch64_instructions(bytes, instructions);
    Ok(())
}

fn canonical_integer_bits(
    source: psi_core::ValueId,
    scalar_type: psi_core::IntegerType,
    value: psi_core::IntegerValue,
) -> Result<u64, EmissionError> {
    let bits = crate::integer_bits(source, scalar_type, value)?;
    if matches!(scalar_type.sign(), psi_core::IntegerSign::Signed) && scalar_type.bits() < 64 {
        let width = u32::from(scalar_type.bits());
        let sign = 1_u64 << (width - 1);
        if bits & sign != 0 {
            return Ok(bits | (!0_u64 << width));
        }
    }
    Ok(bits)
}

fn emit_x86_64_unit_scalar_immediate(bytes: &mut Vec<u8>, register: u8, bits: u64) {
    bytes.push(0x48 | ((register >> 3) & 1));
    bytes.push(0xb8 | (register & 7));
    bytes.extend_from_slice(&bits.to_le_bytes());
}

fn emit_aarch64_unit_scalar_immediate(instructions: &mut Vec<u32>, register: u8, bits: u64) {
    for chunk in 0..4 {
        let immediate = ((bits >> (chunk * 16)) & 0xffff) as u32;
        if chunk == 0 || immediate != 0 {
            let base = if chunk == 0 { 0xd280_0000 } else { 0xf280_0000 };
            instructions
                .push(base | ((chunk as u32) << 21) | (immediate << 5) | u32::from(register));
        }
    }
}

fn emit_x86_64_unit_scalar_normalize(
    bytes: &mut Vec<u8>,
    register: u8,
    scalar_type: psi_core::ScalarType,
) {
    let scalar_type = match scalar_type {
        psi_core::ScalarType::Boolean => {
            psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8)
                .expect("u8 Boolean ABI carrier")
        }
        psi_core::ScalarType::Integer(integer) => integer,
        psi_core::ScalarType::IeeeFloat(_) => {
            unreachable!("Unit scalar homes do not admit IEEE float results")
        }
    };
    if scalar_type.bits() == 64 {
        return;
    }
    let high = (register >> 3) & 1;
    let modrm = 0xc0 | ((register & 7) << 3) | (register & 7);
    match (scalar_type.sign(), scalar_type.bits()) {
        (psi_core::IntegerSign::Signed, 8) => {
            bytes.extend_from_slice(&[0x48 | (high << 2) | high, 0x0f, 0xbe, modrm]);
        }
        (psi_core::IntegerSign::Signed, 16) => {
            bytes.extend_from_slice(&[0x48 | (high << 2) | high, 0x0f, 0xbf, modrm]);
        }
        (psi_core::IntegerSign::Signed, 32) => {
            bytes.extend_from_slice(&[0x48 | (high << 2) | high, 0x63, modrm]);
        }
        (psi_core::IntegerSign::Unsigned, 8) => {
            bytes.extend_from_slice(&[0x40 | (high << 2) | high, 0x0f, 0xb6, modrm]);
        }
        (psi_core::IntegerSign::Unsigned, 16) => {
            bytes.extend_from_slice(&[0x40 | (high << 2) | high, 0x0f, 0xb7, modrm]);
        }
        (psi_core::IntegerSign::Unsigned, 32) => {
            bytes.extend_from_slice(&[0x40 | (high << 2) | high, 0x89, modrm]);
        }
        _ => unreachable!("fixed integer width was validated before emission"),
    }
}

fn emit_aarch64_unit_scalar_normalize(
    instructions: &mut Vec<u32>,
    register: u8,
    scalar_type: psi_core::ScalarType,
) {
    let scalar_type = match scalar_type {
        psi_core::ScalarType::Boolean => {
            psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8)
                .expect("u8 Boolean ABI carrier")
        }
        psi_core::ScalarType::Integer(integer) => integer,
        psi_core::ScalarType::IeeeFloat(_) => {
            unreachable!("Unit scalar homes do not admit IEEE float results")
        }
    };
    if scalar_type.bits() == 64 {
        return;
    }
    let base = match scalar_type.sign() {
        psi_core::IntegerSign::Signed => 0x9340_0000,
        psi_core::IntegerSign::Unsigned => 0xd340_0000,
    };
    instructions.push(
        base | (u32::from(scalar_type.bits() - 1) << 10)
            | (u32::from(register) << 5)
            | u32::from(register),
    );
}
