//! Scalar-result calls copy typed arguments and retain their actual result home.
use super::*;
use legalized_operations::LegalizedScalarInstruction;

/// Snapshot the projected pointer, then place its bits in the exact outgoing ABI slot.
pub(super) fn argument_pointer(
    builder: &mut Builder<'_>,
    operation: &legalized_operations::LegalizedScalarInstruction,
    argument_index: usize,
    semantic: &terminal_psi::StructuralArgument,
    target: &target_operations::TargetStructuralArgument,
) -> Result<Option<VirtualRegisterId>, SelectedInstructionError> {
    let pointer = structural::call_pointer(
        builder,
        operation,
        semantic.place,
        target.source_byte_offset,
    )?;
    let pointer = if let Some(length) = target.fixed_array_length {
        structural::fixed_array_argument(builder, operation, semantic.place, pointer, length)?
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
        operation: operation.operation,
        argument_index: argument_index.try_into().map_err(|_| invalid())?,
    };
    builder
        .transport
        .slots
        .push(selected_instructions::SelectedOutgoingArgumentSlot {
            id: slot,
            byte_size: 8,
            alignment: 8,
            abi_stack_byte_offset: stack_byte_offset,
        });
    builder
        .transport
        .memory
        .push(selected_instructions::SelectedMemoryAccess {
            instruction: SelectedInstructionId(
                builder
                    .instructions
                    .len()
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
    builder.emit(
        SelectedInstructionKind::Store64 {
            slot: selected_instructions::FrameStorageSlotId::Outgoing(slot),
            byte_offset: 0,
        },
        builder.constraints.keys.store64.ok_or_else(invalid)?,
        &[pointer],
        SelectedInstructionProvenance {
            operations: vec![operation.operation],
            ..Default::default()
        },
    )?;
    Ok(None)
}
pub(super) fn emit(
    function: usize,
    source: &LegalizedScalarFunction,
    operation: &LegalizedScalarInstruction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    builder: &mut Builder<'_>,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let invalid = || SelectedInstructionError::UnsupportedSourceShape { function };
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
    if !builder.constraints.keys.call_scalar.contains(&key) {
        return Err(invalid());
    }
    crate::selection::scalar_call_abi::validate(
        function,
        source,
        call,
        operation.operation,
        key,
        row(builder.catalog, key)?,
        environment,
    )?;
    let mut operands = Vec::new();
    for (argument_index, argument) in call.arguments.iter().enumerate() {
        if let legalized_operations::LegalizedScalarArgument::Structural { semantic, target } =
            argument
        {
            if semantic.access == StructuralAccess::Owned {
                operands.extend(
                    super::aggregate_argument::argument(
                        source,
                        operation,
                        argument_index,
                        semantic,
                        target,
                        builder,
                    )?
                    .into_iter()
                    .map(|register| (argument_index, register)),
                );
                continue;
            }
            if let Some(pointer) =
                argument_pointer(builder, operation, argument_index, semantic, target)?
            {
                operands.push((argument_index, pointer));
            }
            continue;
        }
        let (_, input, site, argument_type) = builder
            .resolve(argument.scalar_source().ok_or_else(invalid)?)
            .ok_or_else(invalid)?;
        let shape =
            crate::selection::scalar_call_abi::scalar_shape(argument_type).ok_or_else(invalid)?;
        if argument.placement().shape != shape {
            return Err(invalid());
        }
        if super::scalar_stack::argument(
            builder,
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
                &builder.constraints.keys,
            ) {
            let output = builder.register(value, site, argument_type)?;
            builder.registers[output.0 as usize].class = row(builder.catalog, key)?
                .operands
                .get(1)
                .ok_or_else(invalid)?
                .class;
            builder.emit(
                kind,
                key,
                &[input, output],
                SelectedInstructionProvenance {
                    values: vec![value],
                    ..Default::default()
                },
            )?;
            output
        } else {
            builder.copy(input, value, site, argument_type)?
        };
        operands.push((argument_index, output));
    }
    let order = crate::selection::scalar_call_abi::register_argument_order(call);
    operands.sort_by_key(|(argument, _)| order.iter().position(|index| index == argument));
    let mut operands = operands
        .into_iter()
        .map(|(_, register)| register)
        .collect::<Vec<_>>();
    let short_result = builder.register(result.value, result.definition_site, scalar_type)?;
    builder.registers[short_result.0 as usize].class = row(builder.catalog, key)?
        .operands
        .last()
        .ok_or_else(invalid)?
        .class;
    operands.push(short_result);
    builder
        .transport
        .calls
        .push(selected_instructions::SelectedCallContract {
            instruction: SelectedInstructionId(
                builder
                    .instructions
                    .len()
                    .try_into()
                    .map_err(|_| invalid())?,
            ),
            operation: operation.operation,
            call: call.clone(),
            effect: operation.effect,
            ownership: operation.ownership.clone(),
        });
    builder.emit(
        SelectedInstructionKind::CallScalar {
            callee: call.callee,
        },
        key,
        &operands,
        SelectedInstructionProvenance {
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
    let output = builder.register(result.value, result.definition_site, scalar_type)?;
    let (kind, key) = crate::selection::scalar_call_abi::incoming_float_transfer(
        scalar_type,
        &builder.constraints.keys,
    )
    .unwrap_or((
        crate::selection::scalar_call_abi::integer_abi_normalization(scalar_type),
        builder.constraints.keys.copy_i64,
    ));
    builder.emit(
        kind,
        key,
        &[short_result, output],
        SelectedInstructionProvenance {
            values: vec![result.value],
            ..Default::default()
        },
    )?;
    Ok(output)
}
