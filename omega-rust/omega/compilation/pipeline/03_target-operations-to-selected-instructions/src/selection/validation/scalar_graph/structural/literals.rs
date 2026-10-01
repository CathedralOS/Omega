//! Reconstruct a literal's image address and descriptor home from the retained payload.
use super::local_storage::{address, store};
use super::{
    LegalizedScalarInstruction, LegalizedScalarInstructionKind, PlaceId, SelectedInstructionKind,
    VirtualRegisterId,
};
use crate::SelectedInstructionError;
use crate::selection::validation::scalar_graph::Replay;
use crate::selection::validation::scalar_graph::structural::provenance;
use crate::selection::validation::scalar_graph::structural::result;
use selected_instructions::{LocalStorageSlotId, SelectedLocalStorageSlot};
use semantic_vocabulary::{IntegerValue, StructuralPlaceKind};

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
        || !structural_type.shape.is_borrowed_byte_view()
        || replay
            .transport
            .pointers
            .iter()
            .any(|(place, _)| *place == destination.id)
    {
        return Err(replay.invalid());
    }
    let length = u64::try_from(bytes.len()).map_err(|_| replay.invalid())?;
    let slot = LocalStorageSlotId::Structural {
        operation: row.operation,
        place: destination.id,
    };
    replay.transport.local_slots.push(SelectedLocalStorageSlot {
        id: slot,
        byte_size: 16,
        alignment: 8,
    });
    let backing = result(replay, destination.id, 16)?;
    replay.check_instruction(
        SelectedInstructionKind::ByteLiteralImageAddress {
            operation: row.operation,
            place: destination.id,
        },
        replay
            .constraints
            .keys
            .byte_literal_image_address
            .ok_or_else(|| replay.invalid())?,
        &[backing],
        &provenance(row),
    )?;
    store(replay, row, slot, 0, backing)?;
    let length = constant(replay, row, destination.id, 8, length)?;
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
