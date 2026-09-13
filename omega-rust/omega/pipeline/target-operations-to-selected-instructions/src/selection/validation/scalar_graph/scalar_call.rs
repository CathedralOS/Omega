//! Independent replay of scalar-result calls and their argument transport.
use super::*;
use legalized_operations::{LegalizedScalarArgument, LegalizedScalarInstruction};
use register_environment::ValidatedTargetRegisterEnvironment;

/// Snapshot the projected pointer, then place its bits in the exact outgoing ABI slot.
pub(super) fn argument_pointer(
    replay: &mut Replay<'_>,
    operation: &legalized_operations::LegalizedScalarInstruction,
    argument_index: usize,
    semantic: &terminal_psi::StructuralArgument,
    target: &target_operations::TargetStructuralArgument,
) -> Result<Option<VirtualRegisterId>, SelectedInstructionError> {
    let pointer =
        structural::call_pointer(replay, operation, semantic.place, target.source_byte_offset)?;
    let pointer = if let Some(length) = target.fixed_array_length {
        structural::fixed_array_argument(replay, operation, semantic.place, pointer, length)?
    } else {
        pointer
    };
    let Some(stack_byte_offset) =
        crate::structural_reference_input::stack_pointer_offset(&target.destination)
    else {
        return Ok(Some(pointer));
    };
    let invalid = || SelectedInstructionError::SourceCustodyMismatch;
    let slot = selected_instructions::OutgoingArgumentSlotId {
        role: selected_instructions::OutgoingArgumentSlotRole::Argument,
        operation: operation.operation,
        argument_index: argument_index.try_into().map_err(|_| invalid())?,
    };
    replay
        .transport
        .slots
        .push(selected_instructions::SelectedOutgoingArgumentSlot {
            id: slot,
            byte_size: 8,
            alignment: 8,
            abi_stack_byte_offset: stack_byte_offset,
        });
    replay
        .transport
        .memory
        .push(selected_instructions::SelectedMemoryAccess {
            instruction: SelectedInstructionId(
                replay
                    .instruction_cursor
                    .try_into()
                    .map_err(|_| invalid())?,
            ),
            origin: selected_instructions::SelectedMemoryAccessOrigin::Operation(
                operation.operation,
            ),
            place: semantic.place,
            byte_offset: 0,
            byte_count: 8,
            role: selected_instructions::SelectedMemoryAccessRole::WriteOutgoing { slot },
        });
    replay.check_instruction(
        SelectedInstructionKind::Store64 {
            slot: selected_instructions::FrameStorageSlotId::Outgoing(slot),
            byte_offset: 0,
        },
        replay.constraints.keys.store64.ok_or_else(invalid)?,
        &[pointer],
        &SelectedInstructionProvenance {
            operations: vec![operation.operation],
            ..Default::default()
        },
    )?;
    Ok(None)
}
pub(super) fn validate(
    source: &LegalizedScalarFunction,
    operation: &LegalizedScalarInstruction,
    replay: &mut Replay<'_>,
    environment: &ValidatedTargetRegisterEnvironment,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let function = replay.function;
    let invalid = || SelectedInstructionError::FunctionProjectionMismatch { function };
    let LegalizedScalarInstructionKind::Call(call) = &operation.kind else {
        return Err(invalid());
    };
    let result = operation.result.ok_or_else(invalid)?;
    let scalar_type = result.scalar_type;
    let result_shape =
        crate::selection::scalar_call_abi::scalar_shape(scalar_type).ok_or_else(invalid)?;
    if call
        .result_placement
        .as_ref()
        .map(|placement| placement.shape)
        != Some(result_shape)
    {
        return Err(invalid());
    }
    let key = crate::selection::scalar_call_abi::unit_key(call, environment).ok_or_else(invalid)?;
    if !replay.constraints.keys.call_scalar.contains(&key) {
        return Err(invalid());
    }
    crate::selection::scalar_call_abi::validate(
        function,
        source,
        call,
        operation.operation,
        key,
        row(catalog, key)?,
        environment,
    )?;
    let mut operands = Vec::new();
    for (argument_index, argument) in call.arguments.iter().enumerate() {
        if let LegalizedScalarArgument::Structural { semantic, target } = argument {
            if semantic.access == StructuralAccess::Owned {
                operands.extend(
                    super::aggregate_argument::argument(
                        source,
                        operation,
                        argument_index,
                        semantic,
                        target,
                        replay,
                    )?
                    .into_iter()
                    .map(|register| (argument_index, register)),
                );
                continue;
            }
            if let Some(pointer) =
                argument_pointer(replay, operation, argument_index, semantic, target)?
            {
                operands.push((argument_index, pointer));
            }
            continue;
        }
        let (_, input, site, argument_type) = replay
            .resolve(argument.scalar_source().ok_or_else(invalid)?)
            .ok_or_else(invalid)?;
        let shape =
            crate::selection::scalar_call_abi::scalar_shape(argument_type).ok_or_else(invalid)?;
        if argument.placement().shape != shape {
            return Err(invalid());
        }
        if super::scalar_stack::argument(
            replay,
            operation,
            argument_index,
            argument.scalar_source().ok_or_else(invalid)?,
            argument.placement(),
        )? {
            continue;
        }
        let value = argument.scalar_source().ok_or_else(invalid)?;
        let output = if let Some((kind, key)) =
            crate::selection::scalar_call_abi::outgoing_float_transfer(
                argument_type,
                &replay.constraints.keys,
            ) {
            let class = row(catalog, key)?
                .operands
                .get(1)
                .ok_or_else(invalid)?
                .class;
            let output = replay.check_register_class(
                class,
                site,
                argument_type,
                VirtualRegisterOrigin::InstructionResult {
                    instruction: SelectedInstructionId(
                        replay
                            .instruction_cursor
                            .try_into()
                            .map_err(|_| invalid())?,
                    ),
                    source_value: value,
                },
                None,
            )?;
            replay.check_instruction(
                kind,
                key,
                &[input, output],
                &SelectedInstructionProvenance {
                    values: vec![value],
                    ..Default::default()
                },
            )?;
            output
        } else {
            replay.check_copy(input, value, site, argument_type)?
        };
        operands.push((argument_index, output));
    }
    let order = crate::selection::scalar_call_abi::register_argument_order(call);
    operands.sort_by_key(|(argument, _)| order.iter().position(|index| index == argument));
    let mut operands = operands
        .into_iter()
        .map(|(_, register)| register)
        .collect::<Vec<_>>();
    let class = row(catalog, key)?
        .operands
        .last()
        .ok_or_else(invalid)?
        .class;
    let short_result = replay.check_register_class(
        class,
        result.definition_site,
        scalar_type,
        VirtualRegisterOrigin::InstructionResult {
            instruction: SelectedInstructionId(
                replay
                    .instruction_cursor
                    .try_into()
                    .map_err(|_| invalid())?,
            ),
            source_value: result.value,
        },
        None,
    )?;
    operands.push(short_result);
    replay
        .transport
        .calls
        .push(selected_instructions::SelectedCallContract {
            instruction: SelectedInstructionId(
                replay
                    .instruction_cursor
                    .try_into()
                    .map_err(|_| invalid())?,
            ),
            operation: operation.operation,
            call: call.clone(),
            effect: operation.effect,
            ownership: operation.ownership.clone(),
        });
    replay.check_instruction(
        SelectedInstructionKind::CallScalar {
            callee: call.callee,
        },
        key,
        &operands,
        &SelectedInstructionProvenance {
            operations: vec![operation.operation],
            values: call
                .arguments
                .iter()
                .filter_map(|argument| argument.scalar_source())
                .chain(std::iter::once(result.value))
                .collect(),
            obligations: call.requirement_obligations.clone(),
            fuel: operation.fuel.clone(),
            ..Default::default()
        },
    )?;
    let output = replay.result_register(result.value, result.definition_site, scalar_type)?;
    let (kind, key) = crate::selection::scalar_call_abi::incoming_float_transfer(
        scalar_type,
        &replay.constraints.keys,
    )
    .unwrap_or((
        crate::selection::scalar_call_abi::integer_abi_normalization(scalar_type),
        replay.constraints.keys.copy_i64,
    ));
    replay.check_instruction(
        kind,
        key,
        &[short_result, output],
        &SelectedInstructionProvenance {
            values: vec![result.value],
            ..Default::default()
        },
    )?;
    Ok(output)
}
