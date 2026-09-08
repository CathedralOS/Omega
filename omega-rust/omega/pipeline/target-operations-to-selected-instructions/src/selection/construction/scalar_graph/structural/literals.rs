//! Establish original immutable literal backing and its addressable descriptor.
use super::*;
use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId, SelectedLocalStorageSlot};
use semantic_vocabulary::{IntegerValue, StructuralPlaceKind};
use terminal_psi::{ByteSequenceCarrier, StructuralTypeShape};

pub(super) fn establish(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::EstablishByteSequenceLiteral {
        destination,
        structural_type,
        bytes,
    } = &row.kind
    else {
        return Err(invalid());
    };
    if row.result.is_some()
        || !matches!(destination.kind, StructuralPlaceKind::ByteSequenceLiteral { structural_type: identity, .. } if identity == structural_type.id)
        || structural_type.shape
            != StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView)
        || builder
            .transport
            .pointers
            .iter()
            .any(|(place, _)| *place == destination.id)
    {
        return Err(invalid());
    }
    let length = u32::try_from(bytes.len()).map_err(|_| invalid())?;
    let padded = length.checked_add(7).ok_or_else(invalid)? & !7;
    let byte_size = padded.checked_add(16).ok_or_else(invalid)?;
    let slot = LocalStorageSlotId {
        operation: row.operation,
        place: destination.id,
    };
    builder
        .transport
        .local_slots
        .push(SelectedLocalStorageSlot {
            id: slot,
            byte_size,
            alignment: 8,
        });
    for (chunk_index, chunk) in bytes.chunks(8).enumerate() {
        let offset = u32::try_from(chunk_index)
            .ok()
            .and_then(|value| value.checked_mul(8))
            .and_then(|value| value.checked_add(16))
            .ok_or_else(invalid)?;
        let mut word = [0_u8; 8];
        word[..chunk.len()].copy_from_slice(chunk);
        let value = constant(builder, row, offset, u64::from_le_bytes(word))?;
        store(builder, row, slot, offset, value)?;
    }
    let backing = address(builder, row, slot, 16, length, false)?;
    store(builder, row, slot, 0, backing)?;
    let length = constant(builder, row, 8, u64::from(length))?;
    store(builder, row, slot, 8, length)?;
    let descriptor = address(builder, row, slot, 0, 16, true)?;
    builder
        .transport
        .pointers
        .push((destination.id, descriptor));
    Ok(())
}

fn constant(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    offset: u32,
    value: u64,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let LegalizedScalarInstructionKind::EstablishByteSequenceLiteral { destination, .. } =
        &row.kind
    else {
        return Err(invalid());
    };
    let register = transport_register(builder, destination.id, offset)?;
    builder.emit(
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(u128::from(value)),
        },
        builder.constraints.keys.materialize_i64,
        &[register],
        provenance(row),
    )?;
    Ok(register)
}

fn store(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    slot: LocalStorageSlotId,
    offset: u32,
    value: VirtualRegisterId,
) -> Result<(), SelectedInstructionError> {
    memory(
        builder,
        row,
        slot.place,
        offset,
        8,
        SelectedMemoryAccessRole::WriteLocal { slot },
    )?;
    builder.emit(
        SelectedInstructionKind::Store64 {
            slot: FrameStorageSlotId::Local(slot),
            byte_offset: offset,
        },
        builder.constraints.keys.store64.ok_or_else(invalid)?,
        &[value],
        provenance(row),
    )
}

fn address(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    slot: LocalStorageSlotId,
    offset: u32,
    byte_count: u32,
    settles_fuel: bool,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let register = transport_register(builder, slot.place, offset)?;
    memory(
        builder,
        row,
        slot.place,
        offset,
        byte_count,
        SelectedMemoryAccessRole::AddressLocal { slot },
    )?;
    let mut provenance = provenance(row);
    if settles_fuel {
        provenance.fuel = row.fuel.clone();
    }
    builder.emit(
        SelectedInstructionKind::FrameAddress {
            slot: FrameStorageSlotId::Local(slot),
            byte_offset: offset,
        },
        builder.constraints.keys.frame_address.ok_or_else(invalid)?,
        &[register],
        provenance,
    )?;
    Ok(register)
}
