//! Independent scalar-call replay for attached Unit bodies.
//!
//! The emitter retains semantic sources, ABI destinations, homes, and byte
//! intervals. This module regenerates the physical bytes from those semantic
//! fields and the callee ABI; producer-authored expected bytes are deliberately
//! absent.

use calling_conventions::{
    CallSignature, CallingPolicy, IndirectPointerLocation, MachineRegister, ValueLocation,
    ValuePlacement, ValueShape, evaluate_call_plan,
};
use machine_code::{
    InternalUnitScalarArgumentSourceRecord, InternalUnitScalarCallRecord, MachineCodeFunction,
    SemanticCodeAttribution, SemanticCodeSite, UnitIntegerConstantRecord,
};
use semantic_vocabulary::{
    IntegerCarrier, IntegerSign, IntegerType, IntegerValue, MachineId, ScalarType,
};
use target::{Architecture, NativeTarget};
use target_operations::{CallSiteOwner, FixedIntegerScalarFunctionAbi};

use super::instruction_loads::{
    aarch64_terminal_register, expected_aarch64_stack_load, expected_x86_stack_load,
    x86_terminal_register,
};
use super::{ObjectError, ObjectUnitCallStack, ObjectUnitStack};

pub(super) fn validate_internal_unit_scalar_calls(
    target: NativeTarget,
    function: &MachineCodeFunction,
    functions: &std::collections::BTreeMap<MachineId, &MachineCodeFunction>,
    validated_function_stack: Option<&ObjectUnitStack>,
    validated_call_stacks: &[ObjectUnitCallStack],
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidInternalUnitScalarCallEvidence(function.machine);
    validate_constants(function)?;

    let foreign_result_count = function
        .foreign_calls
        .iter()
        .filter(|call| call.scalar_result.is_some())
        .count();
    if function.internal_unit_scalar_calls.is_empty()
        && function.dynamic_calls.is_empty()
        && function.stored_dynamic_calls.is_empty()
        && function.forwarded_dynamic_descriptor_calls.is_empty()
        && foreign_result_count == 0
    {
        if !function.unit_scalar_homes.is_empty() {
            return Err(invalid());
        }
        return Ok(());
    }
    if function.attachment.is_none()
        || function.fixed_integer_scalar_abi.is_some()
        || function.unit_stack.is_none()
        || function.scalar_stack.is_some()
        || function.internal_unit_scalar_calls.windows(2).any(|pair| {
            pair[0].operation_ordinal >= pair[1].operation_ordinal
                || pair[0].code_offset >= pair[1].code_offset
        })
    {
        return Err(invalid());
    }
    let stack = validated_function_stack.ok_or_else(invalid)?;
    validate_home_roster(target, function, stack)?;

    for call in &function.internal_unit_scalar_calls {
        let callee = functions.get(&call.target).copied().ok_or_else(invalid)?;
        let abi = callee
            .fixed_integer_scalar_abi
            .as_ref()
            .ok_or_else(invalid)?;
        validate_fixed_integer_scalar_abi(target, abi).map_err(|_| invalid())?;
        let call_stack = validated_call_stacks
            .iter()
            .find(|stack| stack.owner == call.owner && stack.target == call.target)
            .ok_or_else(invalid)?;
        validate_call(target, function, abi, stack, call_stack, call)?;
    }
    Ok(())
}

fn validate_constants(function: &MachineCodeFunction) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidInternalUnitScalarCallEvidence(function.machine);
    let mut operations = std::collections::BTreeSet::new();
    let mut values = std::collections::BTreeSet::new();
    let mut previous_ordinal = None;
    for constant in &function.unit_integer_constants {
        if previous_ordinal.is_some_and(|ordinal| ordinal >= constant.operation_ordinal)
            || !operations.insert(constant.defining_operation)
            || !values.insert(constant.source_value)
            || integer_shape(constant.scalar_type).is_none()
            || !constant.scalar_type.admits(constant.value)
            || !function
                .provenance
                .operations
                .contains(&constant.defining_operation)
            || exact_attribution_count(
                &function.semantic_code_attribution,
                constant.defining_operation,
                constant.operation_ordinal,
                None,
            ) != 1
        {
            return Err(invalid());
        }
        previous_ordinal = Some(constant.operation_ordinal);
    }
    Ok(())
}

fn validate_home_roster(
    target: NativeTarget,
    function: &MachineCodeFunction,
    stack: &ObjectUnitStack,
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidInternalUnitScalarCallEvidence(function.machine);
    let mut producers = function
        .internal_unit_scalar_calls
        .iter()
        .map(|call| (call.operation_ordinal, call.owner, &call.result))
        .chain(function.foreign_calls.iter().filter_map(|call| {
            call.scalar_result
                .as_ref()
                .map(|result| (call.operation_ordinal, call.owner, result))
        }))
        .chain(function.dynamic_calls.iter().filter_map(|call| {
            call.result.as_ref().map(|result| {
                (
                    call.operation_ordinal,
                    CallSiteOwner::Operation(call.psi_operation),
                    result,
                )
            })
        }))
        .chain(function.stored_dynamic_calls.iter().map(|call| {
            (
                call.operation_ordinal,
                CallSiteOwner::Operation(call.psi_operation),
                &call.result,
            )
        }))
        .chain(
            function
                .forwarded_dynamic_descriptor_calls
                .iter()
                .filter_map(|call| {
                    call.result.as_ref().map(|result| {
                        (
                            call.operation_ordinal,
                            CallSiteOwner::Operation(call.psi_operation),
                            result,
                        )
                    })
                }),
        )
        .collect::<Vec<_>>();
    producers.sort_by_key(|(ordinal, _, _)| *ordinal);
    if function.unit_scalar_homes.len() != producers.len()
        || producers.windows(2).any(|pair| pair[0].0 >= pair[1].0)
    {
        return Err(invalid());
    }
    let mut cursor = 0_u32;
    for home in &function.unit_parameter_homes {
        let alignment = match target.architecture {
            Architecture::X86_64 => 8,
            Architecture::Aarch64 => u32::from(home.shape.alignment.clamp(8, 16)),
        };
        cursor = align(cursor, alignment).ok_or_else(invalid)?;
        let indirect = matches!(
            home.source.locations.as_slice(),
            [ValueLocation::Indirect { .. }]
        );
        if home.location.stack_byte_offset() != Some(cursor) || home.indirect != indirect {
            return Err(invalid());
        }
        cursor = cursor
            .checked_add(if indirect {
                8
            } else {
                u32::from(home.shape.byte_size)
            })
            .ok_or_else(invalid)?;
    }

    let mut operations = std::collections::BTreeSet::new();
    let mut values = std::collections::BTreeSet::new();
    operations.extend(
        function
            .unit_integer_constants
            .iter()
            .map(|constant| constant.defining_operation),
    );
    values.extend(
        function
            .unit_integer_constants
            .iter()
            .map(|constant| constant.source_value),
    );
    enum Allocation<'a> {
        Dynamic(&'a machine_code::DynamicCallRecord),
        StoredDynamic(&'a machine_code::StoredDynamicCallRecord),
        Scalar(
            CallSiteOwner,
            &'a machine_code::InternalUnitScalarCallResultRecord,
        ),
    }
    let mut allocations = function
        .dynamic_calls
        .iter()
        .map(|call| (call.operation_ordinal, 0_u8, Allocation::Dynamic(call)))
        .chain(function.stored_dynamic_calls.iter().map(|call| {
            (
                call.establishment.operation_ordinal,
                0_u8,
                Allocation::StoredDynamic(call),
            )
        }))
        .chain(
            producers
                .into_iter()
                .map(|(ordinal, owner, result)| (ordinal, 1_u8, Allocation::Scalar(owner, result))),
        )
        .collect::<Vec<_>>();
    allocations.sort_by_key(|(ordinal, phase, _)| (*ordinal, *phase));
    let mut scalar_homes = function.unit_scalar_homes.iter();
    for (_, _, allocation) in allocations {
        match allocation {
            Allocation::Dynamic(dynamic) => {
                cursor = align(cursor, 8).ok_or_else(invalid)?;
                if dynamic.descriptor_home_byte_offset != cursor
                    || dynamic.descriptor_abi.total_byte_size != 16
                    || dynamic.descriptor_abi.byte_alignment != 8
                {
                    return Err(invalid());
                }
                cursor = cursor.checked_add(16).ok_or_else(invalid)?;
            }
            Allocation::StoredDynamic(dynamic) => {
                cursor = align(cursor, 8).ok_or_else(invalid)?;
                if dynamic.establishment.descriptor_home_byte_offset != cursor
                    || dynamic.establishment.descriptor_abi.total_byte_size != 16
                    || dynamic.establishment.descriptor_abi.byte_alignment != 8
                {
                    return Err(invalid());
                }
                cursor = cursor.checked_add(16).ok_or_else(invalid)?;
            }
            Allocation::Scalar(owner, result) => {
                let home = scalar_homes.next().ok_or_else(invalid)?;
                cursor = align(cursor, 8).ok_or_else(invalid)?;
                if home.byte_offset != cursor
                    || result.home != *home
                    || owner != CallSiteOwner::Operation(home.defining_operation)
                    || scalar_home_shape(home.scalar_type) != Some(home.shape)
                    || !operations.insert(home.defining_operation)
                    || !values.insert(home.source_value)
                {
                    return Err(invalid());
                }
                cursor = cursor.checked_add(8).ok_or_else(invalid)?;
            }
        }
    }
    if scalar_homes.next().is_some() {
        return Err(invalid());
    }
    let expected_frame = match target.architecture {
        Architecture::X86_64 => {
            let homes = align(cursor, 16).ok_or_else(invalid)?;
            if function.x86_floating_control.is_some() || !function.foreign_calls.is_empty() {
                homes.checked_add(16).ok_or_else(invalid)?
            } else {
                homes
            }
        }
        Architecture::Aarch64 => {
            let floating_control_slot = align(cursor, 8).ok_or_else(invalid)?;
            let link_offset = floating_control_slot
                .checked_add(if function.foreign_calls.is_empty() {
                    0
                } else {
                    8
                })
                .ok_or_else(invalid)?;
            if function
                .unit_stack
                .and_then(|evidence| evidence.aarch64_return_link)
                .is_none_or(|link| link.frame_byte_offset != link_offset)
            {
                return Err(invalid());
            }
            let frame_end = link_offset.checked_add(8).ok_or_else(invalid)?;
            align(frame_end, 16).ok_or_else(invalid)?
        }
    };
    if expected_frame != stack.frame_bytes {
        return Err(invalid());
    }
    Ok(())
}

fn validate_fixed_integer_scalar_abi(
    target: NativeTarget,
    abi: &FixedIntegerScalarFunctionAbi,
) -> Result<(), ()> {
    let parameter_shapes = abi
        .parameters
        .iter()
        .map(|value| integer_shape(value.scalar_type).ok_or(()))
        .collect::<Result<Vec<_>, _>>()?;
    let result_shape = integer_shape(abi.result.scalar_type).ok_or(())?;
    let expected = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: parameter_shapes,
            result: Some(result_shape),
        },
    )
    .map_err(|_| ())?;
    if expected != abi.call_plan
        || abi.parameters.len() != abi.call_plan.parameters.len()
        || abi
            .parameters
            .iter()
            .zip(&abi.call_plan.parameters)
            .any(|(value, placement)| value.placement != *placement)
        || abi.result.placement != *abi.call_plan.result.as_ref().ok_or(())?
    {
        return Err(());
    }
    Ok(())
}

fn validate_call(
    target: NativeTarget,
    function: &MachineCodeFunction,
    abi: &FixedIntegerScalarFunctionAbi,
    function_stack: &ObjectUnitStack,
    call_stack: &ObjectUnitCallStack,
    call: &InternalUnitScalarCallRecord,
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidInternalUnitScalarCallEvidence(function.machine);
    let CallSiteOwner::Operation(operation) = call.owner else {
        return Err(invalid());
    };
    let relocation = function
        .internal_calls
        .iter()
        .find(|relocation| relocation.owner == call.owner && relocation.target == call.target)
        .ok_or_else(invalid)?;
    if call.call_plan != abi.call_plan
        || call.arguments.len() != abi.parameters.len()
        || call.result.source != abi.result.placement
        || call.result.home.scalar_type != ScalarType::Integer(abi.result.scalar_type)
        || !function.provenance.operations.contains(&operation)
        || exact_attribution_count(
            &function.semantic_code_attribution,
            operation,
            call.operation_ordinal,
            Some((call.code_offset, call.byte_count)),
        ) != 1
        || call_stack.text_offset != relocation.offset
    {
        return Err(invalid());
    }

    let outbound = relocation.unit_stack.ok_or_else(invalid)?.outbound;
    let outbound_bytes = outbound.map_or(0, |outbound| outbound.byte_size);
    let native_call_start = match target.architecture {
        Architecture::X86_64 => relocation.offset.checked_sub(1).ok_or_else(invalid)?,
        Architecture::Aarch64 => relocation.offset,
    };
    let native_call_end = relocation.offset.checked_add(4).ok_or_else(invalid)?;
    let mut cursor = if let Some(outbound) = outbound {
        if call.code_offset != outbound.allocation_offset {
            return Err(invalid());
        }
        outbound
            .allocation_offset
            .checked_add(outbound.allocation_byte_count)
            .ok_or_else(invalid)?
    } else {
        call.code_offset
    };

    for (index, (argument, parameter)) in call.arguments.iter().zip(&abi.parameters).enumerate() {
        if argument.parameter_index != index as u32
            || argument.destination != parameter.placement
            || argument.source.scalar_type() != ScalarType::Integer(parameter.scalar_type)
            || argument.code_offset != cursor
        {
            return Err(invalid());
        }
        validate_source(
            function,
            call.operation_ordinal,
            call.code_offset,
            argument.source,
        )?;
        let expected = expected_argument_bytes(
            target,
            &call.call_plan,
            &call.arguments,
            index,
            function_stack.frame_bytes,
            outbound_bytes,
        )
        .ok_or_else(invalid)?;
        if argument.byte_count != expected.len()
            || function
                .bytes
                .get(cursor..cursor.checked_add(expected.len()).ok_or_else(invalid)?)
                != Some(expected.as_slice())
        {
            return Err(invalid());
        }
        cursor = cursor.checked_add(expected.len()).ok_or_else(invalid)?;
    }
    if cursor != native_call_start {
        return Err(invalid());
    }
    cursor = native_call_end;
    if let Some(outbound) = outbound {
        if outbound.release_offset != cursor {
            return Err(invalid());
        }
        cursor = outbound
            .release_offset
            .checked_add(outbound.release_byte_count)
            .ok_or_else(invalid)?;
    }
    if call.result.code_offset != cursor {
        return Err(invalid());
    }
    let expected_result =
        expected_unit_scalar_result_bytes(target, &call.result).ok_or_else(invalid)?;
    let result_end = cursor
        .checked_add(expected_result.len())
        .ok_or_else(invalid)?;
    let call_end = call
        .code_offset
        .checked_add(call.byte_count)
        .ok_or_else(invalid)?;
    if call.result.byte_count != expected_result.len()
        || result_end != call_end
        || function.bytes.get(cursor..result_end) != Some(expected_result.as_slice())
    {
        return Err(invalid());
    }
    Ok(())
}

pub(super) fn validate_source(
    function: &MachineCodeFunction,
    consumer_operation_ordinal: usize,
    consumer_code_offset: usize,
    source: InternalUnitScalarArgumentSourceRecord,
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidInternalUnitScalarCallEvidence(function.machine);
    match source {
        InternalUnitScalarArgumentSourceRecord::Parameter {
            parameter_index,
            source_value,
            scalar_type,
            location,
        } => {
            let index = usize::try_from(parameter_index).map_err(|_| invalid())?;
            let parameter = function
                .unit_scalar_abi
                .as_ref()
                .and_then(|abi| abi.parameters.get(index))
                .ok_or_else(invalid)?;
            let expected_location = match parameter.placement.locations.as_slice() {
                [
                    calling_conventions::ValueLocation::Register {
                        register,
                        value_byte_offset: 0,
                        byte_size,
                    },
                ] if *byte_size == parameter.placement.shape.byte_size => {
                    machine_code::UnitScalarParameterLocationRecord::Register(*register)
                }
                [
                    calling_conventions::ValueLocation::Stack {
                        stack_byte_offset,
                        value_byte_offset: 0,
                        byte_size,
                        ..
                    },
                ] if *byte_size == parameter.placement.shape.byte_size => {
                    machine_code::UnitScalarParameterLocationRecord::IncomingStack {
                        byte_offset: *stack_byte_offset,
                    }
                }
                _ => return Err(invalid()),
            };
            if parameter.value != source_value
                || parameter.scalar_type != scalar_type
                || location != expected_location
            {
                return Err(invalid());
            }
        }
        InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            defining_operation,
            source_value,
            scalar_type,
            value,
        } => {
            let matches = function
                .unit_integer_constants
                .iter()
                .filter(|constant| {
                    **constant
                        == (UnitIntegerConstantRecord {
                            defining_operation,
                            source_value,
                            scalar_type,
                            value,
                            operation_ordinal: constant.operation_ordinal,
                        })
                        && constant.operation_ordinal < consumer_operation_ordinal
                })
                .count();
            if matches != 1 {
                return Err(invalid());
            }
        }
        InternalUnitScalarArgumentSourceRecord::BooleanImmediate {
            defining_operation,
            source_value,
            value,
            definition_ordinal,
        } => {
            let consistent = function
                .internal_unit_calls
                .iter()
                .flat_map(|call| &call.scalar_arguments)
                .all(|argument| match argument.source {
                    InternalUnitScalarArgumentSourceRecord::BooleanImmediate {
                        defining_operation: candidate_operation,
                        source_value: candidate_value,
                        value: candidate_literal,
                        definition_ordinal: candidate_ordinal,
                    } if candidate_operation == defining_operation
                        || candidate_value == source_value =>
                    {
                        candidate_operation == defining_operation
                            && candidate_value == source_value
                            && candidate_literal == value
                            && candidate_ordinal == definition_ordinal
                    }
                    _ => true,
                });
            if definition_ordinal >= consumer_operation_ordinal
                || !function.provenance.operations.contains(&defining_operation)
                || exact_attribution_count(
                    &function.semantic_code_attribution,
                    defining_operation,
                    definition_ordinal,
                    None,
                ) != 1
                || !consistent
            {
                return Err(invalid());
            }
        }
        InternalUnitScalarArgumentSourceRecord::Home(home) => {
            let matches = exact_preceding_unit_scalar_home_producer_count(
                function,
                home,
                consumer_operation_ordinal,
                consumer_code_offset,
            );
            if matches != 1 || !function.unit_scalar_homes.contains(&home) {
                return Err(invalid());
            }
        }
    }
    Ok(())
}

/// Count exact durable-home producers that are complete before one consumer.
/// Internal and normalized foreign scalar results share this query so neither
/// object validator can admit a weaker or mechanism-specific provenance path.
pub(super) fn exact_preceding_unit_scalar_home_producer_count(
    function: &MachineCodeFunction,
    home: machine_code::UnitScalarHomeRecord,
    consumer_operation_ordinal: usize,
    consumer_code_offset: usize,
) -> usize {
    let internal = function
        .internal_unit_scalar_calls
        .iter()
        .filter(|producer| {
            producer.result.home == home
                && producer.operation_ordinal < consumer_operation_ordinal
                && producer
                    .result
                    .code_offset
                    .checked_add(producer.result.byte_count)
                    .is_some_and(|end| end <= consumer_code_offset)
        })
        .count();
    let foreign = function
        .foreign_calls
        .iter()
        .filter(|producer| {
            producer.operation_ordinal < consumer_operation_ordinal
                && producer.scalar_result.as_ref().is_some_and(|result| {
                    result.home == home
                        && result
                            .code_offset
                            .checked_add(result.byte_count)
                            .is_some_and(|end| end <= consumer_code_offset)
                })
        })
        .count();
    let dynamic = function
        .dynamic_calls
        .iter()
        .filter(|producer| {
            producer.result.as_ref().is_some_and(|result| {
                result.home == home
                    && producer.operation_ordinal < consumer_operation_ordinal
                    && result
                        .code_offset
                        .checked_add(result.byte_count)
                        .is_some_and(|end| end <= consumer_code_offset)
            })
        })
        .count();
    let stored_dynamic = function
        .stored_dynamic_calls
        .iter()
        .filter(|producer| {
            producer.result.home == home
                && producer.operation_ordinal < consumer_operation_ordinal
                && producer
                    .result
                    .code_offset
                    .checked_add(producer.result.byte_count)
                    .is_some_and(|end| end <= consumer_code_offset)
        })
        .count();
    let forwarded = function
        .forwarded_dynamic_descriptor_calls
        .iter()
        .filter(|producer| {
            producer.result.as_ref().is_some_and(|result| {
                result.home == home
                    && producer.operation_ordinal < consumer_operation_ordinal
                    && result
                        .code_offset
                        .checked_add(result.byte_count)
                        .is_some_and(|end| end <= consumer_code_offset)
            })
        })
        .count();
    internal + foreign + dynamic + stored_dynamic + forwarded
}

/// Count exact ordinary internal Unit scalar producers that complete before a
/// runtime boundary consumer. Installation retains this producer family in
/// full; other home-producing call families remain closed for that boundary.
pub(super) fn exact_preceding_internal_unit_scalar_home_producer_count(
    calls: &[machine_code::InternalUnitScalarCallRecord],
    home: machine_code::UnitScalarHomeRecord,
    consumer_operation_ordinal: usize,
    consumer_code_offset: usize,
) -> usize {
    calls
        .iter()
        .filter(|producer| {
            producer.result.home == home
                && producer.operation_ordinal < consumer_operation_ordinal
                && producer
                    .result
                    .code_offset
                    .checked_add(producer.result.byte_count)
                    .is_some_and(|end| end <= consumer_code_offset)
        })
        .count()
}

struct ReplayScalarTransportPlan {
    call_stack_bytes: u32,
    snapshot_slots: Vec<(MachineRegister, u32)>,
}

impl ReplayScalarTransportPlan {
    fn snapshot_offset(&self, register: MachineRegister) -> Option<u32> {
        self.snapshot_slots
            .iter()
            .find_map(|(candidate, offset)| (*candidate == register).then_some(*offset))
    }
}

fn replay_scalar_transport_plan(
    target: NativeTarget,
    call_plan: &calling_conventions::CallPlan,
    arguments: &[machine_code::InternalUnitScalarCallArgumentRecord],
) -> Option<ReplayScalarTransportPlan> {
    let source_registers = arguments
        .iter()
        .filter_map(|argument| match argument.source {
            InternalUnitScalarArgumentSourceRecord::Parameter {
                location: machine_code::UnitScalarParameterLocationRecord::Register(register),
                ..
            } => Some(register),
            _ => None,
        })
        .collect::<Vec<_>>();
    let needs_snapshot = arguments.iter().any(|argument| {
        let Some((Some(destination), None)) =
            placement_destination(&argument.destination, target.architecture)
        else {
            return false;
        };
        let destination_is_source = match target.architecture {
            Architecture::X86_64 => source_registers
                .iter()
                .any(|source| x86_terminal_register(*source) == Some(destination)),
            Architecture::Aarch64 => source_registers
                .iter()
                .any(|source| aarch64_terminal_register(*source) == Some(destination)),
        };
        destination_is_source
            && !matches!(
                argument.source,
                InternalUnitScalarArgumentSourceRecord::Parameter {
                    location:
                        machine_code::UnitScalarParameterLocationRecord::Register(source),
                    ..
                } if match target.architecture {
                    Architecture::X86_64 => x86_terminal_register(source) == Some(destination),
                    Architecture::Aarch64 => aarch64_terminal_register(source) == Some(destination),
                }
            )
    });
    let registers = if needs_snapshot {
        source_registers
            .into_iter()
            .fold(Vec::new(), |mut registers, register| {
                if !registers.contains(&register) {
                    registers.push(register);
                }
                registers
            })
    } else {
        Vec::new()
    };
    let outgoing_bytes = call_plan.parameters.iter().try_fold(
        u32::from(call_plan.shadow_bytes),
        |extent, placement| {
            replay_outgoing_extent(placement, target.architecture).map(|end| extent.max(end))
        },
    )?;
    let mut cursor = outgoing_bytes;
    let mut snapshot_slots = Vec::new();
    if !registers.is_empty() {
        cursor = align_u32(cursor, 8)?;
        for register in registers {
            snapshot_slots.push((register, cursor));
            cursor = cursor.checked_add(8)?;
        }
    }
    let call_stack_bytes = match target.architecture {
        Architecture::X86_64 => {
            let padding = (8 + 16 - (cursor % 16)) % 16;
            cursor.checked_add(padding)?
        }
        Architecture::Aarch64 => align_u32(cursor, 16)?,
    };
    Some(ReplayScalarTransportPlan {
        call_stack_bytes,
        snapshot_slots,
    })
}

fn replay_outgoing_extent(placement: &ValuePlacement, architecture: Architecture) -> Option<u32> {
    placement
        .locations
        .iter()
        .try_fold(0_u32, |extent, location| {
            let end = match *location {
                ValueLocation::Register { .. } => 0,
                ValueLocation::Stack {
                    stack_byte_offset,
                    byte_size,
                    ..
                } => stack_byte_offset.checked_add(u32::from(byte_size.max(8)))?,
                ValueLocation::Indirect {
                    pointer,
                    copy_stack_byte_offset,
                    byte_size,
                    ..
                } => {
                    let pointer_end = match pointer {
                        IndirectPointerLocation::Register(_) => 0,
                        IndirectPointerLocation::Stack {
                            stack_byte_offset, ..
                        } => stack_byte_offset.checked_add(8)?,
                    };
                    if architecture == Architecture::Aarch64 {
                        match copy_stack_byte_offset {
                            Some(offset) => pointer_end
                                .max(offset.checked_add(u32::from(byte_size).next_multiple_of(8))?),
                            None => pointer_end,
                        }
                    } else {
                        pointer_end
                    }
                }
            };
            Some(extent.max(end))
        })
}

fn align_u32(value: u32, alignment: u32) -> Option<u32> {
    let remainder = value % alignment;
    value.checked_add(if remainder == 0 {
        0
    } else {
        alignment - remainder
    })
}

fn expected_x86_register_move(bytes: &mut Vec<u8>, source: u8, destination: u8) {
    if source == destination {
        return;
    }
    bytes.extend_from_slice(&[
        0x48 | (((source >> 3) & 1) << 2) | ((destination >> 3) & 1),
        0x89,
        0xc0 | ((source & 7) << 3) | (destination & 7),
    ]);
}

pub(super) fn expected_argument_bytes(
    target: NativeTarget,
    call_plan: &calling_conventions::CallPlan,
    arguments: &[machine_code::InternalUnitScalarCallArgumentRecord],
    argument_index: usize,
    frame_bytes: u32,
    outbound_bytes: u32,
) -> Option<Vec<u8>> {
    let argument = arguments.get(argument_index)?;
    let transport = replay_scalar_transport_plan(target, call_plan, arguments)?;
    if transport.call_stack_bytes != outbound_bytes {
        return None;
    }
    match target.architecture {
        Architecture::X86_64 => {
            let (register, stack) =
                placement_destination(&argument.destination, target.architecture)?;
            let register = register.unwrap_or(11);
            let mut bytes = Vec::new();
            if argument_index == 0 {
                for (source, offset) in &transport.snapshot_slots {
                    expected_x86_stack_store(&mut bytes, x86_terminal_register(*source)?, *offset);
                }
            }
            let mut value_register = register;
            match argument.source {
                InternalUnitScalarArgumentSourceRecord::Parameter { location, .. } => {
                    match location {
                        machine_code::UnitScalarParameterLocationRecord::Register(source) => {
                            let source_register = x86_terminal_register(source)?;
                            if let Some(offset) = transport.snapshot_offset(source) {
                                expected_x86_stack_load(&mut bytes, register, offset, 8)?;
                            } else if stack.is_some() {
                                value_register = source_register;
                            } else {
                                expected_x86_register_move(&mut bytes, source_register, register);
                            }
                        }
                        machine_code::UnitScalarParameterLocationRecord::IncomingStack {
                            byte_offset,
                        } => expected_x86_stack_load(
                            &mut bytes,
                            register,
                            outbound_bytes
                                .checked_add(frame_bytes)?
                                .checked_add(8)?
                                .checked_add(byte_offset)?,
                            scalar_home_shape(argument.source.scalar_type())?.byte_size,
                        )?,
                    }
                }
                InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
                    scalar_type,
                    value,
                    ..
                } => expected_x86_immediate(
                    &mut bytes,
                    register,
                    canonical_bits(scalar_type, value)?,
                ),
                InternalUnitScalarArgumentSourceRecord::BooleanImmediate { value, .. } => {
                    expected_x86_immediate(&mut bytes, register, u64::from(value))
                }
                InternalUnitScalarArgumentSourceRecord::Home(home) => expected_x86_stack_load(
                    &mut bytes,
                    register,
                    outbound_bytes.checked_add(home.byte_offset)?,
                    8,
                )?,
            }
            if let Some(offset) = stack {
                expected_x86_stack_store(&mut bytes, value_register, offset);
            }
            Some(bytes)
        }
        Architecture::Aarch64 => {
            let (register, stack) =
                placement_destination(&argument.destination, target.architecture)?;
            let register = register.unwrap_or(9);
            let mut words = Vec::new();
            if argument_index == 0 {
                for (source, offset) in &transport.snapshot_slots {
                    words.push(expected_aarch64_stack_store(
                        aarch64_terminal_register(*source)?,
                        *offset,
                    )?);
                }
            }
            let mut value_register = register;
            match argument.source {
                InternalUnitScalarArgumentSourceRecord::Parameter { location, .. } => {
                    match location {
                        machine_code::UnitScalarParameterLocationRecord::Register(source) => {
                            let source_register = aarch64_terminal_register(source)?;
                            if let Some(offset) = transport.snapshot_offset(source) {
                                words.push(expected_aarch64_stack_load(register, offset, 8)?);
                            } else if stack.is_some() {
                                value_register = source_register;
                            } else if source_register != register {
                                words.push(
                                    0xaa00_03e0
                                        | (u32::from(source_register) << 16)
                                        | u32::from(register),
                                );
                            }
                        }
                        machine_code::UnitScalarParameterLocationRecord::IncomingStack {
                            byte_offset,
                        } => words.push(expected_aarch64_stack_load(
                            register,
                            outbound_bytes
                                .checked_add(frame_bytes)?
                                .checked_add(byte_offset)?,
                            scalar_home_shape(argument.source.scalar_type())?.byte_size,
                        )?),
                    }
                }
                InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
                    scalar_type,
                    value,
                    ..
                } => expected_aarch64_immediate(
                    &mut words,
                    register,
                    canonical_bits(scalar_type, value)?,
                ),
                InternalUnitScalarArgumentSourceRecord::BooleanImmediate { value, .. } => {
                    expected_aarch64_immediate(&mut words, register, u64::from(value))
                }
                InternalUnitScalarArgumentSourceRecord::Home(home) => {
                    words.push(expected_aarch64_stack_load(
                        register,
                        outbound_bytes.checked_add(home.byte_offset)?,
                        8,
                    )?)
                }
            }
            if let Some(offset) = stack {
                words.push(expected_aarch64_stack_store(value_register, offset)?);
            }
            Some(words.into_iter().flat_map(u32::to_le_bytes).collect())
        }
    }
}

pub(super) fn expected_unit_scalar_result_bytes(
    target: NativeTarget,
    result: &machine_code::InternalUnitScalarCallResultRecord,
) -> Option<Vec<u8>> {
    let (register, stack) = placement_destination(&result.source, target.architecture)?;
    if stack.is_some() {
        return None;
    }
    let register = register?;
    match target.architecture {
        Architecture::X86_64 => {
            let mut bytes = Vec::new();
            expected_x86_normalize(&mut bytes, register, result.home.scalar_type);
            expected_x86_stack_store(&mut bytes, register, result.home.byte_offset);
            Some(bytes)
        }
        Architecture::Aarch64 => {
            let mut words = Vec::new();
            expected_aarch64_normalize(&mut words, register, result.home.scalar_type);
            words.push(expected_aarch64_stack_store(
                register,
                result.home.byte_offset,
            )?);
            Some(words.into_iter().flat_map(u32::to_le_bytes).collect())
        }
    }
}

fn placement_destination(
    placement: &ValuePlacement,
    architecture: Architecture,
) -> Option<(Option<u8>, Option<u32>)> {
    match placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] if *byte_size == placement.shape.byte_size => Some((
            Some(match architecture {
                Architecture::X86_64 => x86_terminal_register(*register)?,
                Architecture::Aarch64 => aarch64_terminal_register(*register)?,
            }),
            None,
        )),
        [
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset: 0,
                byte_size,
                ..
            },
        ] if *byte_size == placement.shape.byte_size => Some((None, Some(*stack_byte_offset))),
        _ => None,
    }
}

pub(super) fn integer_shape(integer: IntegerType) -> Option<ValueShape> {
    if integer.carrier() != IntegerCarrier::Fixed || !matches!(integer.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let bytes = integer.bits().div_ceil(8);
    Some(ValueShape::integer(bytes, bytes.next_power_of_two().min(8)))
}

pub(super) fn scalar_home_shape(scalar: ScalarType) -> Option<ValueShape> {
    match scalar {
        ScalarType::Boolean => Some(ValueShape::integer(1, 1)),
        ScalarType::Integer(integer) => integer_shape(integer),
        ScalarType::IeeeFloat(_) => None,
    }
}

fn canonical_bits(integer: IntegerType, value: IntegerValue) -> Option<u64> {
    if integer_shape(integer).is_none() || !integer.admits(value) {
        return None;
    }
    Some(match (integer.sign(), value) {
        (IntegerSign::Signed, IntegerValue::Signed(value)) => value as u64,
        (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) => value as u64,
        _ => return None,
    })
}

fn expected_x86_immediate(bytes: &mut Vec<u8>, register: u8, bits: u64) {
    bytes.push(0x48 | ((register >> 3) & 1));
    bytes.push(0xb8 | (register & 7));
    bytes.extend_from_slice(&bits.to_le_bytes());
}

pub(super) fn expected_x86_stack_store(bytes: &mut Vec<u8>, register: u8, offset: u32) {
    bytes.push(0x48 | (((register >> 3) & 1) << 2));
    bytes.push(0x89);
    if offset <= i8::MAX as u32 {
        bytes.extend_from_slice(&[0x44 | ((register & 7) << 3), 0x24, offset as u8]);
    } else {
        bytes.extend_from_slice(&[0x84 | ((register & 7) << 3), 0x24]);
        bytes.extend_from_slice(&offset.to_le_bytes());
    }
}

fn expected_x86_normalize(bytes: &mut Vec<u8>, register: u8, scalar: ScalarType) {
    let integer = match scalar {
        ScalarType::Boolean => {
            IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 Boolean ABI carrier")
        }
        ScalarType::Integer(integer) => integer,
        ScalarType::IeeeFloat(_) => return,
    };
    if integer.bits() == 64 {
        return;
    }
    let high = (register >> 3) & 1;
    let modrm = 0xc0 | ((register & 7) << 3) | (register & 7);
    match (integer.sign(), integer.bits()) {
        (IntegerSign::Signed, 8) => {
            bytes.extend_from_slice(&[0x48 | (high << 2) | high, 0x0f, 0xbe, modrm])
        }
        (IntegerSign::Signed, 16) => {
            bytes.extend_from_slice(&[0x48 | (high << 2) | high, 0x0f, 0xbf, modrm])
        }
        (IntegerSign::Signed, 32) => {
            bytes.extend_from_slice(&[0x48 | (high << 2) | high, 0x63, modrm])
        }
        (IntegerSign::Unsigned, 8) => {
            bytes.extend_from_slice(&[0x40 | (high << 2) | high, 0x0f, 0xb6, modrm])
        }
        (IntegerSign::Unsigned, 16) => {
            bytes.extend_from_slice(&[0x40 | (high << 2) | high, 0x0f, 0xb7, modrm])
        }
        (IntegerSign::Unsigned, 32) => {
            bytes.extend_from_slice(&[0x40 | (high << 2) | high, 0x89, modrm])
        }
        _ => {}
    }
}

fn expected_aarch64_immediate(words: &mut Vec<u32>, register: u8, bits: u64) {
    for chunk in 0..4 {
        let immediate = ((bits >> (chunk * 16)) & 0xffff) as u32;
        if chunk == 0 || immediate != 0 {
            let base = if chunk == 0 { 0xd280_0000 } else { 0xf280_0000 };
            words.push(base | ((chunk as u32) << 21) | (immediate << 5) | u32::from(register));
        }
    }
}

pub(super) fn expected_aarch64_stack_store(register: u8, offset: u32) -> Option<u32> {
    (offset.is_multiple_of(8) && offset / 8 <= 0xfff)
        .then_some(0xf900_0000 | ((offset / 8) << 10) | (31 << 5) | u32::from(register))
}

fn expected_aarch64_normalize(words: &mut Vec<u32>, register: u8, scalar: ScalarType) {
    let integer = match scalar {
        ScalarType::Boolean => {
            IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 Boolean ABI carrier")
        }
        ScalarType::Integer(integer) => integer,
        ScalarType::IeeeFloat(_) => return,
    };
    if integer.bits() == 64 {
        return;
    }
    let base = match integer.sign() {
        IntegerSign::Signed => 0x9340_0000,
        IntegerSign::Unsigned => 0xd340_0000,
    };
    words.push(
        base | (u32::from(integer.bits() - 1) << 10)
            | (u32::from(register) << 5)
            | u32::from(register),
    );
}

fn exact_attribution_count(
    attribution: &[SemanticCodeAttribution],
    operation: semantic_vocabulary::OperationId,
    ordinal: usize,
    interval: Option<(usize, usize)>,
) -> usize {
    attribution
        .iter()
        .filter(|row| {
            row.site == SemanticCodeSite::Operation(operation)
                && row.operation_ordinal == ordinal
                && interval.is_none_or(|(offset, count)| {
                    row.code_offset == offset && row.byte_count == count
                })
                && (interval.is_some() || row.byte_count == 0)
        })
        .count()
}

fn align(value: u32, alignment: u32) -> Option<u32> {
    if alignment == 0 || !alignment.is_power_of_two() {
        return None;
    }
    value
        .checked_add(alignment - 1)
        .map(|value| value & !(alignment - 1))
}
