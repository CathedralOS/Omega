//! Addressable activation-local storage shared by literal and derived descriptors.
use super::*;
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
