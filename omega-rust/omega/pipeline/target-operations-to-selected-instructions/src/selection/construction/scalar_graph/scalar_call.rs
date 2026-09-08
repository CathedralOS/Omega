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
    let key = builder
        .constraints
        .keys
        .call_i64
        .get(crate::selection::scalar_call_abi::register_argument_count(
            call,
        ))
        .copied()
        .ok_or_else(invalid)?;
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
            if let Some(pointer) =
                argument_pointer(builder, operation, argument_index, semantic, target)?
            {
                operands.push(pointer);
            }
            continue;
        }
        let (_, input, site, argument_type) = builder
            .resolve(argument.scalar_source().ok_or_else(invalid)?)
            .ok_or_else(invalid)?;
        let shape = match argument_type {
            ScalarType::Boolean => calling_conventions::ValueShape::integer(1, 1),
            ScalarType::Integer(integer) if integer.bits() == 64 => {
                calling_conventions::ValueShape::integer(8, 8)
            }
            _ => return Err(invalid()),
        };
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
        operands.push(builder.copy(
            input,
            argument.scalar_source().ok_or_else(invalid)?,
            site,
            argument_type,
        )?);
    }
    let short_result = builder.register(result.value, result.definition_site, scalar_type)?;
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
        SelectedInstructionKind::CallI64 {
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
    builder.copy(
        short_result,
        result.value,
        result.definition_site,
        scalar_type,
    )
}
