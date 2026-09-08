//! Preserve a boundary's owned sum result in operation-owned frame storage.
use super::*;
use legalized_operations::LegalizedScalarInstruction;
use selected_instructions::{
    LocalStorageSlotId, SelectedBoundarySettlement, SelectedBoundarySettlementPayload,
    SelectedLocalStorageSlot,
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
    let invalid = || SelectedInstructionError::SourceCustodyMismatch;
    if !row.has_valid_hosted_read_byte_shape()
        || row.result.is_some()
        || !matches!(row.ownership.as_slice(), [optimization_unit::OwnershipEvent::ClaimCompletion(claims)] if claims.is_empty())
    {
        return Err(invalid());
    }
    let slot = LocalStorageSlotId::Structural {
        operation: row.operation,
        place: result.place,
    };
    replay.transport.local_slots.push(SelectedLocalStorageSlot {
        id: slot,
        byte_size: u32::from(layout.shape.byte_size),
        alignment: layout.shape.alignment,
    });
    let instruction_index = replay.block_cursor.try_into().map_err(|_| invalid())?;
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
            .ok_or_else(invalid)?,
        &[],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(true)
}
