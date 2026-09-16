//! Addressable activation-local storage shared by literal and derived descriptors.
use super::{
    Builder, LegalizedScalarInstruction, PlaceId, SelectedInstructionKind,
    SelectedMemoryAccessRole, VirtualRegisterId, memory,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::invalid;
use crate::selection::construction::scalar_graph::structural::provenance;
use crate::selection::construction::scalar_graph::structural::transport_register;
use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId};

/// Stage only the descriptor; its backing remains the caller's original array.
pub(in crate::selection::construction::scalar_graph) fn fixed_array_argument(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    backing: VirtualRegisterId,
    length: u64,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let slot = LocalStorageSlotId::Structural {
        operation: row.operation,
        place,
    };
    builder
        .transport
        .local_slots
        .push(selected_instructions::SelectedLocalStorageSlot {
            id: slot,
            byte_size: 16,
            alignment: 8,
        });
    store(builder, row, slot, 0, backing)?;
    let count = transport_register(builder, place, 8)?;
    builder.emit(
        SelectedInstructionKind::MaterializeI64 {
            value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(length)),
        },
        builder.constraints.keys.materialize_i64,
        &[count],
        provenance(row),
    )?;
    store(builder, row, slot, 8, count)?;
    address(builder, row, slot, 0, 16, false)
}

/// Stage only the descriptor; the field's live length word and bytes remain in
/// the caller's record. `field` points at the inline storage itself, so the
/// length is loaded there and the data begins eight bytes later — the declared
/// capacity is never the descriptor's length.
pub(in crate::selection::construction::scalar_graph) fn byte_field_argument(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    field: VirtualRegisterId,
    field_offset: u32,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let slot = LocalStorageSlotId::Structural {
        operation: row.operation,
        place,
    };
    builder
        .transport
        .local_slots
        .push(selected_instructions::SelectedLocalStorageSlot {
            id: slot,
            byte_size: 16,
            alignment: 8,
        });
    let length = transport_register(builder, place, field_offset)?;
    memory(
        builder,
        row,
        place,
        field_offset,
        8,
        SelectedMemoryAccessRole::ReadPlace,
    )?;
    builder.emit(
        SelectedInstructionKind::Load64 { byte_offset: 0 },
        builder.constraints.keys.load64.ok_or_else(invalid)?,
        &[field, length],
        provenance(row),
    )?;
    let data = transport_register(
        builder,
        place,
        field_offset.checked_add(8).ok_or_else(invalid)?,
    )?;
    builder.emit(
        SelectedInstructionKind::AddressOffset { byte_offset: 8 },
        builder
            .constraints
            .keys
            .address_offset
            .ok_or_else(invalid)?,
        &[field, data],
        provenance(row),
    )?;
    store(builder, row, slot, 0, data)?;
    store(builder, row, slot, 8, length)?;
    address(builder, row, slot, 0, 16, false)
}

pub(super) fn store(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    slot: LocalStorageSlotId,
    offset: u32,
    value: VirtualRegisterId,
) -> Result<(), SelectedInstructionError> {
    memory(
        builder,
        row,
        slot.structural_place().ok_or_else(invalid)?,
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

pub(super) fn address(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    slot: LocalStorageSlotId,
    offset: u32,
    byte_count: u32,
    settles_fuel: bool,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let register = transport_register(
        builder,
        slot.structural_place().ok_or_else(invalid)?,
        offset,
    )?;
    memory(
        builder,
        row,
        slot.structural_place().ok_or_else(invalid)?,
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
