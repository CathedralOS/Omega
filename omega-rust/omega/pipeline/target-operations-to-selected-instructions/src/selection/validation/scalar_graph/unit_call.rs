//! Independently replay resultless argument transport and the exact Unit call row.
use super::*;
use legalized_operations::{LegalizedScalarArgument, LegalizedScalarInstruction};
use selected_instructions::SelectedCallContract;

pub(super) fn validate(
    source: &LegalizedScalarFunction,
    operation: &LegalizedScalarInstruction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    replay: &mut Replay<'_>,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::Call(call) = &operation.kind else {
        return Err(replay.invalid());
    };
    if operation.result.is_some()
        || (call.result_placement.is_some() && call.structural_result.is_none())
    {
        return Err(replay.invalid());
    }
    call.validate_source(&operation.ownership)
        .map_err(|_| replay.invalid())?;
    let key = crate::selection::scalar_call_abi::unit_key(call, environment)
        .ok_or_else(|| replay.invalid())?;
    let constraint = environment
        .constraint(key)
        .ok_or_else(|| replay.invalid())?;
    crate::selection::scalar_call_abi::validate(
        replay.function,
        source,
        call,
        operation.operation,
        key,
        constraint,
        environment,
    )?;
    let mut operands = Vec::new();
    for (argument_index, argument) in call.arguments.iter().enumerate() {
        match argument {
            LegalizedScalarArgument::Structural { semantic, target } => {
                if semantic.access == StructuralAccess::Owned {
                    for register in super::aggregate_argument::argument(
                        source, operation, semantic, target, replay,
                    )? {
                        operands.push((argument_index, register));
                    }
                    continue;
                }
                if let Some(pointer) = super::scalar_call::argument_pointer(
                    replay,
                    operation,
                    argument_index,
                    semantic,
                    target,
                )? {
                    operands.push((argument_index, pointer));
                }
            }
            LegalizedScalarArgument::Scalar {
                source: value,
                placement,
            } => {
                if super::scalar_stack::argument(
                    replay,
                    operation,
                    argument_index,
                    *value,
                    placement,
                )? {
                    continue;
                }
                let (_, input, site, scalar_type) =
                    replay.resolve(*value).ok_or_else(|| replay.invalid())?;
                if crate::selection::scalar_call_abi::scalar_shape(scalar_type)
                    != Some(placement.shape)
                {
                    return Err(replay.invalid());
                }
                let output = if let Some((kind, key)) =
                    crate::selection::scalar_call_abi::outgoing_float_transfer(
                        scalar_type,
                        &replay.constraints.keys,
                    ) {
                    let class = environment
                        .constraint(key)
                        .and_then(|row| row.operands.get(1))
                        .ok_or_else(|| replay.invalid())?
                        .class;
                    let output = replay.check_register_class(
                        class,
                        site,
                        scalar_type,
                        VirtualRegisterOrigin::InstructionResult {
                            instruction: SelectedInstructionId(
                                replay
                                    .instruction_cursor
                                    .try_into()
                                    .map_err(|_| replay.invalid())?,
                            ),
                            source_value: *value,
                        },
                        None,
                    )?;
                    replay.check_instruction(
                        kind,
                        key,
                        &[input, output],
                        &SelectedInstructionProvenance {
                            values: vec![*value],
                            ..Default::default()
                        },
                    )?;
                    output
                } else {
                    replay.check_copy(input, *value, site, scalar_type)?
                };
                operands.push((argument_index, output));
            }
        }
    }
    let order = crate::selection::scalar_call_abi::register_argument_order(call);
    let mut operands = order
        .iter()
        .flat_map(|index| {
            operands
                .iter()
                .filter(move |(argument, _)| argument == index)
                .map(|(_, register)| *register)
        })
        .collect::<Vec<_>>();
    let mut result_registers = Vec::new();
    if let Some(result) = &call.structural_result {
        for location in &call
            .result_placement
            .as_ref()
            .ok_or_else(|| replay.invalid())?
            .locations
        {
            let ValueLocation::Register {
                value_byte_offset, ..
            } = location
            else {
                return Err(replay.invalid());
            };
            let register = super::structural_case::temporary(
                replay,
                result.place,
                u32::from(*value_byte_offset),
                false,
            )?;
            operands.push(register);
            result_registers.push(register);
        }
    }
    replay.transport.calls.push(SelectedCallContract {
        instruction: SelectedInstructionId(
            replay
                .instruction_cursor
                .try_into()
                .map_err(|_| replay.invalid())?,
        ),
        operation: operation.operation,
        call: call.clone(),
        effect: operation.effect,
        ownership: operation.ownership.clone(),
    });
    replay.check_instruction(
        if call.structural_result.is_some()
            && !call
                .result_placement
                .as_ref()
                .is_some_and(crate::selection::scalar_call_abi::empty_aggregate_placement)
        {
            SelectedInstructionKind::CallAggregate {
                callee: call.callee,
            }
        } else {
            SelectedInstructionKind::CallUnit {
                callee: call.callee,
            }
        },
        key,
        &operands,
        &SelectedInstructionProvenance {
            operations: vec![operation.operation],
            values: call
                .arguments
                .iter()
                .filter_map(|argument| argument.scalar_source())
                .collect(),
            obligations: call.requirement_obligations.clone(),
            fuel: operation.fuel.clone(),
            ..Default::default()
        },
    )?;
    if let Some((result, placement)) =
        crate::selection::aggregate_result_input::call_result(source, call)
    {
        // The call and complete structural result remain retained, but an
        // empty payload owns no physical result home or address operation.
        if crate::selection::scalar_call_abi::empty_aggregate_placement(placement) {
            return Ok(());
        }
        use selected_instructions::{
            FrameStorageSlotId, LocalStorageSlotId, SelectedLocalStorageSlot,
            SelectedMemoryAccessRole,
        };
        let slot = LocalStorageSlotId::Structural {
            operation: operation.operation,
            place: result.place,
        };
        replay.transport.local_slots.push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: u32::from(placement.shape.byte_size),
            alignment: placement.shape.alignment,
        });
        let block = source
            .blocks
            .iter()
            .find(|block| {
                block
                    .instructions
                    .iter()
                    .any(|row| row.operation == operation.operation)
            })
            .ok_or_else(|| replay.invalid())?
            .id;
        let pointer = super::structural_case::temporary(replay, result.place, 0, false)?;
        super::structural_case::memory(
            replay,
            block,
            result.place,
            0,
            u32::from(placement.shape.byte_size),
            SelectedMemoryAccessRole::AddressLocal { slot },
        )?;
        replay.check_instruction(
            SelectedInstructionKind::FrameAddress {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            replay
                .constraints
                .keys
                .frame_address
                .ok_or_else(|| replay.invalid())?,
            &[pointer],
            &Default::default(),
        )?;
        for (location, value) in placement.locations.iter().zip(result_registers) {
            let ValueLocation::Register {
                value_byte_offset,
                byte_size,
                ..
            } = location
            else {
                return Err(replay.invalid());
            };
            super::structural_case::memory(
                replay,
                block,
                result.place,
                u32::from(*value_byte_offset),
                u32::from(*byte_size),
                SelectedMemoryAccessRole::WritePlace,
            )?;
            super::aggregate_memory::store(
                replay,
                pointer,
                value,
                u32::from(*value_byte_offset),
                *byte_size,
            )?;
        }
    }
    Ok(())
}
