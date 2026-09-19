//! Independent replay of evaluated normalized foreign calls: the same
//! operand transport, the same unique per-plan row, and the same roster
//! record are reconstructed and compared, never trusted from the proposal.
use super::{
    LegalizedScalarFunction, LegalizedScalarInstructionKind, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionProvenance, ValueLocation, VirtualRegisterOrigin,
};
use crate::SelectedInstructionError;
use crate::selection::validation::scalar_graph::Replay;
use legalized_operations::LegalizedScalarInstruction;
use selected_instructions::SelectedNormalizedForeignCall;

pub(super) fn validate(
    source: &LegalizedScalarFunction,
    operation: &LegalizedScalarInstruction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    replay: &mut Replay<'_>,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::NormalizedForeignCall(call) = &operation.kind else {
        return Err(replay.invalid());
    };
    if operation.result.is_some() != call.result_home.is_some() {
        return Err(replay.invalid());
    }
    let key = crate::selection::scalar_call_abi::normalized_foreign::call_key(call, environment)
        .ok_or_else(|| replay.invalid())?;
    if !replay
        .constraints
        .keys
        .call_normalized_foreign
        .contains(&key)
    {
        return Err(replay.invalid());
    }
    let constraint = environment
        .constraint(key)
        .ok_or_else(|| replay.invalid())?;
    crate::selection::scalar_call_abi::normalized_foreign::validate(
        replay.function,
        source,
        operation,
        key,
        constraint,
        environment,
    )?;
    let mut operands = Vec::new();
    for argument in &call.scalar_arguments {
        let value = argument.source.source_value();
        if super::scalar_stack::argument(
            replay,
            operation,
            argument.parameter_index as usize,
            value,
            &argument.placement,
        )? {
            continue;
        }
        let (_, input, site, scalar_type) =
            replay.resolve(value).ok_or_else(|| replay.invalid())?;
        if scalar_type != argument.source.scalar_type()
            || crate::selection::scalar_call_abi::scalar_shape(scalar_type)
                != Some(argument.placement.shape)
        {
            return Err(replay.invalid());
        }
        let output = replay.check_copy(input, value, site, scalar_type)?;
        operands.push((argument.parameter_index, output));
    }
    for (argument_index, argument) in call.structural_arguments.iter().enumerate() {
        let pointer = super::structural::place_pointer(
            replay,
            operation,
            argument.place,
            argument.source_byte_offset,
        )?;
        match argument.destination.locations.as_slice() {
            [ValueLocation::Register { .. }] => {
                operands.push((argument_index as u32, pointer));
            }
            [
                ValueLocation::Stack {
                    stack_byte_offset, ..
                },
            ] => {
                let slot = selected_instructions::OutgoingArgumentSlotId {
                    role: selected_instructions::OutgoingArgumentSlotRole::Argument,
                    operation: operation.operation,
                    argument_index: argument_index.try_into().map_err(|_| replay.invalid())?,
                };
                replay
                    .transport
                    .slots
                    .push(selected_instructions::SelectedOutgoingArgumentSlot {
                        id: slot,
                        byte_size: 8,
                        alignment: 8,
                        abi_stack_byte_offset: *stack_byte_offset,
                    });
                replay
                    .transport
                    .memory
                    .push(selected_instructions::SelectedMemoryAccess {
                        instruction: SelectedInstructionId(
                            replay
                                .instruction_cursor
                                .try_into()
                                .map_err(|_| replay.invalid())?,
                        ),
                        origin: selected_instructions::SelectedMemoryAccessOrigin::Operation(
                            operation.operation,
                        ),
                        place: argument.place,
                        byte_offset: 0,
                        byte_count: 8,
                        role: selected_instructions::SelectedMemoryAccessRole::WriteOutgoing {
                            slot,
                        },
                    });
                replay.check_instruction(
                    SelectedInstructionKind::Store64 {
                        slot: selected_instructions::FrameStorageSlotId::Outgoing(slot),
                        byte_offset: 0,
                    },
                    replay
                        .constraints
                        .keys
                        .store64
                        .ok_or_else(|| replay.invalid())?,
                    &[pointer],
                    &SelectedInstructionProvenance {
                        operations: vec![operation.operation],
                        ..Default::default()
                    },
                )?;
            }
            _ => return Err(replay.invalid()),
        }
    }
    operands.sort_by_key(|(position, _)| *position);
    let mut operands: Vec<_> = operands.into_iter().map(|(_, register)| register).collect();
    let short_result = if let Some(result) = operation.result {
        let class = constraint
            .operands
            .last()
            .ok_or_else(|| replay.invalid())?
            .class;
        let register = replay.check_register_class(
            class,
            result.definition_site,
            result.scalar_type,
            VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(
                    replay
                        .instruction_cursor
                        .try_into()
                        .map_err(|_| replay.invalid())?,
                ),
                source_value: result.value,
            },
            None,
        )?;
        operands.push(register);
        Some(register)
    } else {
        None
    };
    let ordinal = replay.transport.normalized_foreign_calls.len();
    replay
        .transport
        .normalized_foreign_calls
        .push(SelectedNormalizedForeignCall {
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
        SelectedInstructionKind::NormalizedForeignCall {
            boundary: call.boundary,
            ordinal: ordinal.try_into().map_err(|_| replay.invalid())?,
        },
        key,
        &operands,
        &SelectedInstructionProvenance {
            operations: vec![operation.operation],
            values: call
                .scalar_arguments
                .iter()
                .map(|argument| argument.source.source_value())
                .chain(operation.result.map(|result| result.value))
                .collect(),
            fuel: operation.fuel.clone(),
            ..Default::default()
        },
    )?;
    if let (Some(short_result), Some(result)) = (short_result, operation.result) {
        let output =
            replay.result_register(result.value, result.definition_site, result.scalar_type)?;
        replay.check_instruction(
            crate::selection::scalar_call_abi::integer_carrier_normalization(result.scalar_type),
            replay.constraints.keys.copy_i64,
            &[short_result, output],
            &SelectedInstructionProvenance {
                values: vec![result.value],
                ..Default::default()
            },
        )?;
        replay.definitions.push((
            result.value,
            output,
            result.definition_site,
            result.scalar_type,
        ));
    }
    Ok(())
}
