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
    let key = replay
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
        row(catalog, key)?,
        environment,
    )?;
    let mut operands = Vec::new();
    for (argument_index, argument) in call.arguments.iter().enumerate() {
        if let LegalizedScalarArgument::Structural { semantic, target } = argument {
            if let Some(pointer) =
                argument_pointer(replay, operation, argument_index, semantic, target)?
            {
                operands.push(pointer);
            }
            continue;
        }
        let (_, input, site, argument_type) = replay
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
            replay,
            operation,
            argument_index,
            argument.scalar_source().ok_or_else(invalid)?,
            argument.placement(),
        )? {
            continue;
        }
        operands.push(replay.check_copy(
            input,
            argument.scalar_source().ok_or_else(invalid)?,
            site,
            argument_type,
        )?);
    }
    let short_result = replay.result_register(result.value, result.definition_site, scalar_type)?;
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
        SelectedInstructionKind::CallI64 {
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
    replay.check_copy(
        short_result,
        result.value,
        result.definition_site,
        scalar_type,
    )
}
