//! Transport an owned array through its exact ABI fragments.
use super::*;
use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId, SelectedMemoryAccessRole};

pub(super) fn argument(
    source: &LegalizedScalarFunction,
    operation: &legalized_operations::LegalizedScalarInstruction,
    argument_index: usize,
    semantic: &terminal_psi::StructuralArgument,
    target: &target_operations::TargetStructuralArgument,
    builder: &mut Builder<'_>,
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
    // Call custody has already rejoined the exact producer or parameter type.
    // No physical home is needed to transport a zero-byte owned value.
    if crate::selection::scalar_call_abi::empty_aggregate_placement(&target.destination)
        && target.shape == target.destination.shape
    {
        return Ok(Vec::new());
    }
    if target.shape != target.destination.shape
        || !crate::selection::aggregate_result_input::inline_argument_fragments(&target.destination)
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
        if builder
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
        let pointer = super::structural_case::register(builder, place, 0, 64, false)?;
        super::structural_case::memory(
            builder,
            block,
            place,
            0,
            u32::from(target.shape.byte_size),
            SelectedMemoryAccessRole::AddressLocal { slot },
        )?;
        builder.emit(
            SelectedInstructionKind::FrameAddress {
                slot: FrameStorageSlotId::Local(slot),
                byte_offset: 0,
            },
            builder.constraints.keys.frame_address.ok_or_else(invalid)?,
            &[pointer],
            Default::default(),
        )?;
        Some(pointer)
    } else {
        None
    };
    let outgoing = if let Some(ValueLocation::Stack {
        stack_byte_offset, ..
    }) = target.destination.locations.first()
    {
        let slot = selected_instructions::OutgoingArgumentSlotId {
            operation: operation.operation,
            argument_index: argument_index.try_into().map_err(|_| invalid())?,
        };
        if builder
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
        builder
            .transport
            .slots
            .push(selected_instructions::SelectedOutgoingArgumentSlot {
                id: slot,
                byte_size: u32::from(target.shape.byte_size),
                alignment,
                abi_stack_byte_offset: *stack_byte_offset,
            });
        let address = super::structural_case::register(builder, place, 0, 64, false)?;
        outgoing_memory(
            builder,
            operation.operation,
            place,
            0,
            u32::from(target.shape.byte_size),
            SelectedMemoryAccessRole::AddressOutgoing { slot },
        )?;
        builder.emit(
            SelectedInstructionKind::FrameAddress {
                slot: FrameStorageSlotId::Outgoing(slot),
                byte_offset: 0,
            },
            builder.constraints.keys.frame_address.ok_or_else(invalid)?,
            &[address],
            Default::default(),
        )?;
        Some((slot, address))
    } else {
        None
    };
    let mut registers = Vec::new();
    for location in &target.destination.locations {
        let (value_byte_offset, byte_size) = match location {
            ValueLocation::Register {
                value_byte_offset,
                byte_size,
                ..
            }
            | ValueLocation::Stack {
                value_byte_offset,
                byte_size,
                ..
            } => (value_byte_offset, byte_size),
            _ => return Err(invalid()),
        };
        let offset = u32::from(*value_byte_offset);
        let output = super::structural_case::register(builder, place, offset, 64, false)?;
        if let Some(pointer) = pointer {
            super::structural_case::memory(
                builder,
                block,
                place,
                offset,
                u32::from(*byte_size),
                SelectedMemoryAccessRole::ReadPlace,
            )?;
            super::aggregate_memory::load(builder, pointer, output, offset, *byte_size)?;
        } else {
            let input = builder
                .transport
                .fragments
                .iter()
                .find(|(owner, position, _)| *owner == place && *position == offset)
                .map(|(_, _, register)| *register)
                .ok_or_else(invalid)?;
            builder.emit(
                SelectedInstructionKind::CopyI64,
                builder.constraints.keys.copy_i64,
                &[input, output],
                Default::default(),
            )?;
        }
        if let Some((slot, address)) = outgoing {
            outgoing_memory(
                builder,
                operation.operation,
                place,
                offset,
                u32::from(*byte_size),
                SelectedMemoryAccessRole::WriteOutgoing { slot },
            )?;
            super::aggregate_memory::store(builder, address, output, offset, *byte_size)?;
        } else {
            registers.push(output);
        }
    }
    Ok(registers)
}

/// Outgoing ABI storage belongs to this call occurrence, not to a new source place.
fn outgoing_memory(
    builder: &mut Builder<'_>,
    operation: semantic_vocabulary::OperationId,
    place: semantic_vocabulary::PlaceId,
    byte_offset: u32,
    byte_count: u32,
    role: SelectedMemoryAccessRole,
) -> Result<(), SelectedInstructionError> {
    builder
        .transport
        .memory
        .push(selected_instructions::SelectedMemoryAccess {
            instruction: SelectedInstructionId(
                builder
                    .instructions
                    .len()
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
