//! Preserve a boundary's owned sum result in operation-owned frame storage.
use super::{
    LegalizedScalarInstructionKind, SelectedInstructionKind, SelectedInstructionProvenance,
};
use crate::SelectedInstructionError;
use crate::selection::validation::scalar_graph::Replay;
use legalized_operations::LegalizedScalarInstruction;
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedBoundarySettlement,
    SelectedBoundarySettlementPayload, SelectedInstructionId, SelectedLocalStorageSlot,
    SelectedMemoryAccess, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole,
};

pub(super) fn validate(
    row: &LegalizedScalarInstruction,
    replay: &mut Replay<'_>,
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
    replay.transport.local_slots.push(SelectedLocalStorageSlot {
        id: slot,
        byte_size: layout.shape.byte_size,
        alignment: layout.shape.alignment,
    });
    let instruction_index = replay
        .block_cursor
        .try_into()
        .map_err(|_| SelectedInstructionError::custody())?;
    replay
        .transport
        .settlements
        .push(SelectedBoundarySettlement {
            block: replay.block.id,
            instruction_index,
            settlement: SelectedBoundarySettlementPayload::HostedReadByte {
                operation: row.operation,
                boundary: *boundary,
                result: result.clone(),
                layout: layout.clone(),
            },
        });
    replay.check_instruction(
        SelectedInstructionKind::HostedReadByte { slot },
        replay
            .constraints
            .keys
            .hosted_read_byte
            .ok_or_else(|| SelectedInstructionError::custody())?,
        &[],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    let pointer = super::structural::result(replay, result.place, 0)?;
    replay.transport.memory.push(SelectedMemoryAccess {
        instruction: SelectedInstructionId(
            replay
                .instruction_cursor
                .try_into()
                .map_err(|_| replay.invalid())?,
        ),
        origin: SelectedMemoryAccessOrigin::Operation(row.operation),
        place: result.place,
        byte_offset: 0,
        byte_count: layout.shape.byte_size,
        role: SelectedMemoryAccessRole::AddressLocal { slot },
    });
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
        &SelectedInstructionProvenance::default(),
    )?;
    replay.transport.pointers.push((result.place, pointer));
    Ok(true)
}
