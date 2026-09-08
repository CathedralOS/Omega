//! Independently rejoin selected output, source input and the exact local scratch slot.
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
    let LegalizedScalarInstructionKind::HostedWriteByteI32 { boundary, source } = row.kind else {
        return Ok(false);
    };
    let (_, input, _, scalar_type) = replay.resolve(source).ok_or_else(|| replay.invalid())?;
    if row.result.is_some()
        || !matches!(row.ownership.as_slice(), [optimization_unit::OwnershipEvent::ClaimCompletion(claims)] if claims.is_empty())
        || !matches!(scalar_type, ScalarType::Integer(integer)
            if integer.bits() == 32 && integer.sign() == IntegerSign::Signed)
    {
        return Err(replay.invalid());
    }
    let slot = LocalStorageSlotId::Boundary {
        operation: row.operation,
    };
    replay.transport.local_slots.push(SelectedLocalStorageSlot {
        id: slot,
        byte_size: 1,
        alignment: 1,
    });
    replay
        .transport
        .settlements
        .push(SelectedBoundarySettlement {
            block: replay.block.id,
            instruction_index: replay
                .block_cursor
                .try_into()
                .map_err(|_| replay.invalid())?,
            settlement: SelectedBoundarySettlementPayload::HostedWriteByteI32 {
                operation: row.operation,
                boundary,
                source,
            },
        });
    replay.check_instruction(
        SelectedInstructionKind::HostedWriteByteI32 { slot },
        replay
            .constraints
            .keys
            .hosted_write_byte_i32
            .ok_or_else(|| replay.invalid())?,
        &[input],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![source],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(true)
}
