//! Transport owned aggregate bytes through the independently selected call ABI.
//! Indirection copies the current value into call-owned backing; forwarding the
//! input pointer would alias a copyable value with the callee's private storage.
use super::*;
use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId, SelectedMemoryAccessRole};

pub(super) fn argument(
    source: &LegalizedScalarFunction,
    operation: &legalized_operations::LegalizedScalarInstruction,
    argument_index: usize,
    semantic: &terminal_psi::StructuralArgument,
    target: &target_operations::TargetStructuralArgument,
    replay: &mut Replay<'_>,
) -> Result<Vec<VirtualRegisterId>, SelectedInstructionError> {
    let invalid = || SelectedInstructionError::SourceCustodyMismatch;
    let place = semantic.place;
    let slot = match target.source {
        target_operations::TargetStructuralArgumentSource::StructuralHome { psi_operation } => {
            Some(LocalStorageSlotId::Structural {
                operation: psi_operation,
                place,
            })
        }
        target_operations::TargetStructuralArgumentSource::Placement(_) => None,
        _ => return Err(invalid()),
    };
    // Exact semantic call replay retains the empty value's producer and type;
    // zero ABI fragments cannot justify a fabricated physical home.
    if crate::selection::scalar_call_abi::empty_aggregate_placement(&target.destination)
        && target.shape == target.destination.shape
    {
        return Ok(Vec::new());
    }
    if target.shape != target.destination.shape
        || !crate::selection::aggregate_result_input::owned_argument_placement(&target.destination)
    {
        return Err(invalid());
    }
    let block = source
        .blocks
        .iter()
        .find(|block| {
            block
                .instructions
                .iter()
                .any(|row| row.operation == operation.operation)
        })
        .ok_or_else(invalid)?
        .id;
    let pointer = if let Some(slot) = slot {
        if replay
            .transport
            .local_slots
            .iter()
            .filter(|home| {
                home.id == slot
                    && home.byte_size == u32::from(target.shape.byte_size)
                    && home.alignment == target.shape.alignment
            })
            .count()
            != 1
        {
            return Err(invalid());
        }
        let pointer = super::structural_case::temporary(replay, place, 0, false)?;
        super::structural_case::memory(
            replay,
            block,
            place,
            0,
            u32::from(target.shape.byte_size),
            SelectedMemoryAccessRole::AddressLocal { slot },
        )?;
        replay.check_instruction(
            SelectedInstructionKind::FrameAddress {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            replay.constraints.keys.frame_address.ok_or_else(invalid)?,
            &[pointer],
            &Default::default(),
        )?;
        Some(pointer)
    } else {
        replay
            .transport
            .pointers
            .iter()
            .find(|(owner, _)| *owner == place)
            .map(|(_, pointer)| *pointer)
    };
    let indirect = crate::selection::aggregate_result_input::indirect_argument(&target.destination);
    let outgoing_offset =
        indirect
            .map(|(_, offset)| offset)
            .or_else(|| match target.destination.locations.first() {
                Some(ValueLocation::Stack {
                    stack_byte_offset, ..
                }) => Some(*stack_byte_offset),
                _ => None,
            });
    let outgoing = if let Some(stack_byte_offset) = outgoing_offset {
        let slot = selected_instructions::OutgoingArgumentSlotId {
            operation: operation.operation,
            argument_index: argument_index.try_into().map_err(|_| invalid())?,
            role: if indirect.is_some() {
                selected_instructions::OutgoingArgumentSlotRole::ValueCopy
            } else {
                selected_instructions::OutgoingArgumentSlotRole::Argument
            },
        };
        if replay
            .transport
            .slots
            .iter()
            .any(|existing| existing.id == slot)
        {
            return Err(invalid());
        }
        let alignment = target
            .destination
            .locations
            .iter()
            .filter_map(|location| {
                if let ValueLocation::Stack { alignment, .. } = location {
                    Some(*alignment)
                } else {
                    None
                }
            })
            .fold(target.shape.alignment, u16::max);
        let alignment = if indirect.is_some()
            && source.call_plan.policy == calling_conventions::CallingPolicy::MicrosoftX64
        {
            alignment.max(16)
        } else {
            alignment
        };
        replay
            .transport
            .slots
            .push(selected_instructions::SelectedOutgoingArgumentSlot {
                id: slot,
                byte_size: u32::from(target.shape.byte_size),
                alignment,
                abi_stack_byte_offset: stack_byte_offset,
            });
        let address = super::structural_case::temporary(replay, place, 0, false)?;
        outgoing_memory(
            replay,
            operation.operation,
            place,
            0,
            u32::from(target.shape.byte_size),
            SelectedMemoryAccessRole::AddressOutgoing { slot },
        )?;
        replay.check_instruction(
            SelectedInstructionKind::FrameAddress {
                slot: FrameStorageSlotId::Outgoing(slot),
                byte_offset: 0,
            },
            replay.constraints.keys.frame_address.ok_or_else(invalid)?,
            &[address],
            &Default::default(),
        )?;
        Some((slot, address))
    } else {
        None
    };
    let mut registers = Vec::new();
    let fragments = if indirect.is_some() {
        (0..target.shape.byte_size)
            .step_by(8)
            .map(|offset| (offset, (target.shape.byte_size - offset).min(8)))
            .collect::<Vec<_>>()
    } else {
        target
            .destination
            .locations
            .iter()
            .map(|location| match location {
                ValueLocation::Register {
                    value_byte_offset,
                    byte_size,
                    ..
                }
                | ValueLocation::Stack {
                    value_byte_offset,
                    byte_size,
                    ..
                } => Ok((*value_byte_offset, *byte_size)),
                _ => Err(invalid()),
            })
            .collect::<Result<Vec<_>, _>>()?
    };
    for (value_byte_offset, byte_size) in fragments {
        let offset = u32::from(value_byte_offset);
        let output = super::structural_case::temporary(replay, place, offset, false)?;
        if let Some(pointer) = pointer {
            super::structural_case::memory(
                replay,
                block,
                place,
                offset,
                u32::from(byte_size),
                SelectedMemoryAccessRole::ReadPlace,
            )?;
            super::aggregate_memory::load(replay, pointer, output, offset, byte_size)?;
        } else {
            let input = replay
                .transport
                .fragments
                .iter()
                .find(|(owner, position, _)| *owner == place && *position == offset)
                .map(|(_, _, register)| *register)
                .ok_or_else(invalid)?;
            replay.check_instruction(
                SelectedInstructionKind::CopyI64,
                replay.constraints.keys.copy_i64,
                &[input, output],
                &Default::default(),
            )?;
        }
        if let Some((slot, address)) = outgoing {
            outgoing_memory(
                replay,
                operation.operation,
                place,
                offset,
                u32::from(byte_size),
                SelectedMemoryAccessRole::WriteOutgoing { slot },
            )?;
            super::aggregate_memory::store(replay, address, output, offset, byte_size)?;
        } else {
            registers.push(output);
        }
    }
    if let Some((pointer_location, _)) = indirect {
        let (_, address) = outgoing.ok_or_else(invalid)?;
        match pointer_location {
            calling_conventions::IndirectPointerLocation::Register(_) => registers.push(address),
            calling_conventions::IndirectPointerLocation::Stack {
                stack_byte_offset,
                alignment,
            } => {
                let slot = selected_instructions::OutgoingArgumentSlotId {
                    operation: operation.operation,
                    argument_index: argument_index.try_into().map_err(|_| invalid())?,
                    role: selected_instructions::OutgoingArgumentSlotRole::Argument,
                };
                if replay
                    .transport
                    .slots
                    .iter()
                    .any(|existing| existing.id == slot)
                {
                    return Err(invalid());
                }
                replay
                    .transport
                    .slots
                    .push(selected_instructions::SelectedOutgoingArgumentSlot {
                        id: slot,
                        byte_size: 8,
                        alignment,
                        abi_stack_byte_offset: stack_byte_offset,
                    });
                outgoing_memory(
                    replay,
                    operation.operation,
                    place,
                    0,
                    8,
                    SelectedMemoryAccessRole::WriteOutgoing { slot },
                )?;
                replay.check_instruction(
                    SelectedInstructionKind::Store64 {
                        slot: FrameStorageSlotId::Outgoing(slot),
                        byte_offset: 0,
                    },
                    replay.constraints.keys.store64.ok_or_else(invalid)?,
                    &[address],
                    &Default::default(),
                )?;
            }
        }
    }
    Ok(registers)
}

/// Outgoing ABI storage belongs to this call occurrence, not to a new source place.
fn outgoing_memory(
    replay: &mut Replay<'_>,
    operation: semantic_vocabulary::OperationId,
    place: semantic_vocabulary::PlaceId,
    byte_offset: u32,
    byte_count: u32,
    role: SelectedMemoryAccessRole,
) -> Result<(), SelectedInstructionError> {
    replay
        .transport
        .memory
        .push(selected_instructions::SelectedMemoryAccess {
            instruction: SelectedInstructionId(
                replay
                    .instruction_cursor
                    .try_into()
                    .map_err(|_| SelectedInstructionError::SourceCustodyMismatch)?,
            ),
            origin: selected_instructions::SelectedMemoryAccessOrigin::Operation(operation),
            place,
            byte_offset,
            byte_count,
            role,
        });
    Ok(())
}
