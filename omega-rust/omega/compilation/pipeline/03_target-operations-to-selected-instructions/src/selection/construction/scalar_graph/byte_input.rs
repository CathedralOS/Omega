//! Preserve a boundary's owned sum result in operation-owned frame storage.
use super::{
    Builder, LegalizedScalarInstructionKind, SelectedBlockId, SelectedInstructionKind,
    SelectedInstructionProvenance,
};
use crate::SelectedInstructionError;
use legalized_operations::LegalizedScalarInstruction;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedBoundarySettlement,
    SelectedBoundarySettlementPayload, SelectedInstructionId, SelectedLocalStorageSlot,
    SelectedMemoryAccess, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole,
};

pub(super) fn emit(
    row: &LegalizedScalarInstruction,
    block: SelectedBlockId,
    block_start: usize,
    builder: &mut Builder<'_>,
) -> Result<bool, SelectedInstructionError> {
    let LegalizedScalarInstructionKind::HostedReadByte {
        boundary,
        result,
        layout,
    } = &row.kind
    else {
        return Ok(false);
    };
    if !row.has_valid_hosted_read_byte_shape()
        || row.result.is_some()
        || !matches!(row.ownership.as_slice(), [optimization_unit::OwnershipEvent::ClaimCompletion(claims)] if claims.is_empty())
    {
        return Err(SelectedInstructionError::custody());
    }
    let slot = LocalStorageSlotId::Structural {
        operation: row.operation,
        place: result.place,
    };
    builder
        .transport
        .local_slots
        .push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: layout.shape.byte_size,
            alignment: layout.shape.alignment,
        });
    let instruction_index = builder
        .instructions
        .len()
        .checked_sub(block_start)
        .ok_or_else(|| SelectedInstructionError::custody())?
        .try_into()
        .map_err(|_| SelectedInstructionError::custody())?;
    builder
        .transport
        .settlements
        .push(SelectedBoundarySettlement {
            block,
            instruction_index,
            settlement: SelectedBoundarySettlementPayload::HostedReadByte {
                operation: row.operation,
                boundary: *boundary,
                result: result.clone(),
                layout: layout.clone(),
            },
        });
    builder.emit(
        SelectedInstructionKind::HostedReadByte { slot },
        builder
            .constraints
            .keys
            .hosted_read_byte
            .ok_or_else(|| SelectedInstructionError::custody())?,
        &[],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    // Publish the slot's address the same way an indirect call result does:
    // projected reads (tag membership, payload fields) resolve the place
    // through `runtime_address` to this home. The address carry is not itself
    // an operation site — the settlement claims this operation's single
    // attribution row for the read-byte span.
    let pointer = super::structural::transport_register(builder, result.place, 0)?;
    builder.transport.memory.push(SelectedMemoryAccess {
        instruction: SelectedInstructionId(
            builder
                .instructions
                .len()
                .try_into()
                .map_err(|_| SelectedInstructionError::custody())?,
        ),
        origin: SelectedMemoryAccessOrigin::Operation(row.operation),
        place: result.place,
        byte_offset: 0,
        byte_count: layout.shape.byte_size,
        role: SelectedMemoryAccessRole::AddressLocal { slot },
    });
    builder.emit(
        SelectedInstructionKind::FrameAddress {
            slot: FrameStorageSlotId::Local(slot),
            byte_offset: 0,
        },
        builder
            .constraints
            .keys
            .frame_address
            .ok_or_else(|| SelectedInstructionError::custody())?,
        &[pointer],
        Default::default(),
    )?;
    builder.transport.pointers.push((result.place, pointer));
    Ok(true)
}
