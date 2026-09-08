//! Reconstruct literal stores and descriptor homes from the retained raw payload.
use super::local_storage::{address, store};
use super::*;
use selected_instructions::{LocalStorageSlotId, SelectedLocalStorageSlot};
use semantic_vocabulary::{IntegerValue, StructuralPlaceKind};
use terminal_psi::{ByteSequenceCarrier, StructuralTypeShape};

pub(super) fn establish(
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::EstablishByteSequenceLiteral {
        destination,
        structural_type,
        bytes,
    } = &row.kind
    else {
        return Err(replay.invalid());
    };
    if row.result.is_some()
        || !matches!(destination.kind, StructuralPlaceKind::ByteSequenceLiteral { structural_type: identity, .. } if identity == structural_type.id)
        || structural_type.shape
            != StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView)
        || replay
            .transport
            .pointers
            .iter()
            .any(|(place, _)| *place == destination.id)
    {
        return Err(replay.invalid());
    }
    let length = u32::try_from(bytes.len()).map_err(|_| replay.invalid())?;
    let padded = length.checked_add(7).ok_or_else(|| replay.invalid())? & !7;
    let byte_size = padded.checked_add(16).ok_or_else(|| replay.invalid())?;
    let slot = LocalStorageSlotId {
        operation: row.operation,
        place: destination.id,
    };
    replay.transport.local_slots.push(SelectedLocalStorageSlot {
        id: slot,
        byte_size,
        alignment: 8,
    });
    for (chunk_index, chunk) in bytes.chunks(8).enumerate() {
        let offset = u32::try_from(chunk_index)
            .ok()
            .and_then(|value| value.checked_mul(8))
            .and_then(|value| value.checked_add(16))
            .ok_or_else(|| replay.invalid())?;
        let mut word = [0_u8; 8];
        word[..chunk.len()].copy_from_slice(chunk);
        let value = constant(
            replay,
            row,
            destination.id,
            offset,
            u64::from_le_bytes(word),
        )?;
        store(replay, row, slot, offset, value)?;
    }
    let backing = address(replay, row, slot, 16, length, false)?;
    store(replay, row, slot, 0, backing)?;
    let length = constant(replay, row, destination.id, 8, u64::from(length))?;
    store(replay, row, slot, 8, length)?;
    let descriptor = address(replay, row, slot, 0, 16, true)?;
    replay.transport.pointers.push((destination.id, descriptor));
    Ok(())
}

fn constant(
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    offset: u32,
    value: u64,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let register = result(replay, place, offset)?;
    replay.check_instruction(
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(u128::from(value)),
        },
        replay.constraints.keys.materialize_i64,
        &[register],
        &provenance(row),
    )?;
    Ok(register)
}
