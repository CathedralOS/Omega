//! Independent replay of outgoing call arguments. Every scalar-graph call,
//! whatever its result, must have placed each argument in its ABI home: an
//! owned aggregate as its register fragments, a borrowed structural argument
//! as a pointer, and a scalar as a copy or float transfer into the call row's
//! class. A stack-placed argument is checked in its outgoing slot and
//! yields no operand.
use super::{
    LegalizedScalarFunction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionProvenance, VirtualRegisterId, VirtualRegisterOrigin,
};
use crate::SelectedInstructionError;
use crate::selection::validation::scalar_graph::Replay;
use crate::selection::validation::scalar_graph::structural;
use legalized_operations::{
    LegalizedScalarArgument, LegalizedScalarCall, LegalizedScalarInstruction,
};
use register_environment::ValidatedTargetRegisterEnvironment;
use terminal_psi::StructuralAccess;

/// Replay every argument of `call` and return the register operands in the
/// call row's register order. `invalid` is the caller's refusal.
pub(super) fn replay(
    source: &LegalizedScalarFunction,
    operation: &LegalizedScalarInstruction,
    call: &LegalizedScalarCall,
    replay: &mut Replay<'_>,
    environment: &ValidatedTargetRegisterEnvironment,
    invalid: impl Fn() -> SelectedInstructionError,
) -> Result<Vec<VirtualRegisterId>, SelectedInstructionError> {
    let mut operands = Vec::new();
    for (argument_index, argument) in call.arguments.iter().enumerate() {
        let (value, placement) = match argument {
            LegalizedScalarArgument::Structural {
                semantic,
                target,
                indices,
            } => {
                if semantic.access != StructuralAccess::Owned
                    && (call.validate_source(&operation.ownership).is_err()
                        || !call.claim_transfers.is_empty()
                        || matches!(call.source, target_operations::NativeCallOrigin::Authored)
                            && !matches!(operation.ownership.as_slice(),
                                [optimization_unit::OwnershipEvent::ClaimTransfer(claims)] if claims.is_empty()))
                {
                    return Err(invalid());
                }
                operands.extend(
                    structural_argument(
                        source,
                        operation,
                        argument_index,
                        semantic,
                        target,
                        indices,
                        replay,
                    )?
                    .into_iter()
                    .map(|register| (argument_index, register)),
                );
                continue;
            }
            LegalizedScalarArgument::Scalar {
                source: value,
                placement,
            } => (*value, placement),
        };
        let (_, input, site, scalar_type) = replay.resolve(value).ok_or_else(&invalid)?;
        if crate::selection::scalar_call_abi::scalar_shape(scalar_type) != Some(placement.shape) {
            return Err(invalid());
        }
        if super::scalar_stack::argument(replay, operation, argument_index, value, placement)? {
            continue;
        }
        let output = if let Some((kind, key)) =
            crate::selection::scalar_call_abi::outgoing_float_transfer(
                scalar_type,
                &replay.constraints.keys,
            ) {
            let class = environment
                .constraint(key)
                .and_then(|row| row.operands.get(1))
                .ok_or_else(&invalid)?
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
            replay.check_copy(input, value, site, scalar_type)?
        };
        operands.push((argument_index, output));
    }
    // Only a register-placed argument yields operands, and the register order
    // names every such argument.
    let order = crate::selection::scalar_call_abi::register_argument_order(call);
    if operands
        .iter()
        .any(|(argument, _)| !order.contains(argument))
    {
        return Err(invalid());
    }
    Ok(order
        .iter()
        .flat_map(|index| {
            operands
                .iter()
                .filter(move |(argument, _)| argument == index)
                .map(|(_, register)| *register)
        })
        .collect())
}

/// Transport a validated structural actual independently of the call mechanism.
/// Ownership selects value bytes versus the original referent; ABI indirection
/// only selects where a private owned copy is placed.
pub(super) fn structural_argument(
    source: &LegalizedScalarFunction,
    operation: &LegalizedScalarInstruction,
    argument_index: usize,
    semantic: &terminal_psi::StructuralArgument,
    target: &target_operations::TargetStructuralArgument,
    indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
    replay: &mut Replay<'_>,
) -> Result<Vec<VirtualRegisterId>, SelectedInstructionError> {
    if semantic.place != target.place
        || semantic.path != target.path
        || semantic.access != target.access
    {
        return Err(SelectedInstructionError::custody());
    }
    if semantic.access == StructuralAccess::Owned {
        return super::aggregate_argument::argument(
            source,
            operation,
            argument_index,
            semantic,
            target,
            replay,
        );
    }
    Ok(argument_pointer(
        replay,
        source,
        operation,
        argument_index,
        semantic,
        target,
        indices,
    )?
    .into_iter()
    .collect())
}

/// Snapshot the projected pointer, then place its bits in the exact outgoing ABI slot.
fn argument_pointer(
    replay: &mut Replay<'_>,
    source: &LegalizedScalarFunction,
    operation: &legalized_operations::LegalizedScalarInstruction,
    argument_index: usize,
    semantic: &terminal_psi::StructuralArgument,
    target: &target_operations::TargetStructuralArgument,
    indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
) -> Result<Option<VirtualRegisterId>, SelectedInstructionError> {
    let referent = semantic
        .path
        .iter()
        .any(|segment| matches!(segment, terminal_psi::StructuralPathSegment::Referent));
    let pointer_place = semantic.place;
    // Rebuild owner presentation from the retained source identity rather
    // than trusting construction's choice of whole or field geometry.
    let byte_field = if referent {
        None
    } else {
        crate::selection::byte_owner_input::view_argument(
            source,
            operation.operation,
            semantic,
            target,
        )
    };
    let pointer = if referent {
        let offset = crate::selection::reference_input::borrowed_argument(source, semantic, target)
            .filter(|_| indices.is_empty())
            .ok_or_else(|| replay.invalid())?;
        structural::reference::borrowed_argument(replay, operation, semantic.place, offset)?
    } else if matches!(
        target.source,
        target_operations::TargetStructuralArgumentSource::EstablishedElementView { .. }
    ) && !semantic.path.is_empty()
    {
        structural::view_element_pointer(
            replay,
            operation,
            pointer_place,
            target.source_byte_offset,
        )?
    } else {
        structural::place_pointer(replay, operation, pointer_place, target.source_byte_offset)?
    };
    let pointer = if indices.is_empty() {
        pointer
    } else {
        structural::runtime_address::scale(
            replay,
            operation,
            pointer_place,
            target.source_byte_offset,
            pointer,
            indices,
        )?
    };
    let pointer = if let Some(length) = target.fixed_array_length {
        structural::fixed_array_argument(
            replay,
            operation,
            semantic.place,
            argument_index
                .try_into()
                .map_err(|_| SelectedInstructionError::custody())?,
            pointer,
            length,
            target.shape.byte_size,
        )?
    } else if let Some((field_offset, _)) = byte_field {
        if field_offset != target.source_byte_offset {
            return Err(SelectedInstructionError::custody());
        }
        structural::byte_field_argument(
            replay,
            operation,
            semantic.place,
            argument_index
                .try_into()
                .map_err(|_| SelectedInstructionError::custody())?,
            pointer,
            field_offset,
        )?
    } else {
        pointer
    };
    let Some(stack_byte_offset) =
        crate::structural_inputs::structural_reference_input::stack_pointer_offset(
            &target.destination,
        )
    else {
        return Ok(Some(pointer));
    };
    let invalid = || SelectedInstructionError::custody();
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
            place: pointer_place,
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
