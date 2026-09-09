//! Addressable activation-local storage shared by literal and derived descriptors.
use super::*;
use selected_instructions::{FrameStorageSlotId, LocalStorageSlotId};

/// Stage only the descriptor; its backing remains the caller's original array.
pub(in crate::selection::validation::scalar_graph) fn fixed_array_argument(
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    backing: VirtualRegisterId,
    length: u64,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let slot = LocalStorageSlotId::Structural {
        operation: row.operation,
        place,
    };
    replay
        .transport
        .local_slots
        .push(selected_instructions::SelectedLocalStorageSlot {
            id: slot,
            byte_size: 16,
            alignment: 8,
        });
    store(replay, row, slot, 0, backing)?;
    let count = result(replay, place, 8)?;
    replay.check_instruction(
        SelectedInstructionKind::MaterializeI64 {
            value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(length)),
        },
        replay.constraints.keys.materialize_i64,
        &[count],
        &provenance(row),
    )?;
    store(replay, row, slot, 8, count)?;
    address(replay, row, slot, 0, 16, false)
}

pub(super) fn store(
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
    slot: LocalStorageSlotId,
    offset: u32,
    value: VirtualRegisterId,
) -> Result<(), SelectedInstructionError> {
    memory(
        replay,
        row,
        slot.structural_place().ok_or_else(|| replay.invalid())?,
        offset,
        8,
        SelectedMemoryAccessRole::WriteLocal { slot },
    )?;
    replay.check_instruction(
        SelectedInstructionKind::Store64 {
            slot: FrameStorageSlotId::Local(slot),
            byte_offset: offset,
        },
        replay
            .constraints
            .keys
            .store64
            .ok_or_else(|| replay.invalid())?,
        &[value],
        &provenance(row),
    )
}

pub(super) fn address(
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
    slot: LocalStorageSlotId,
    offset: u32,
    byte_count: u32,
    settles_fuel: bool,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let register = result(
        replay,
        slot.structural_place().ok_or_else(|| replay.invalid())?,
        offset,
    )?;
    memory(
        replay,
        row,
        slot.structural_place().ok_or_else(|| replay.invalid())?,
        offset,
        byte_count,
        SelectedMemoryAccessRole::AddressLocal { slot },
    )?;
    let mut provenance = provenance(row);
    if settles_fuel {
        provenance.fuel = row.fuel.clone();
    }
    replay.check_instruction(
        SelectedInstructionKind::FrameAddress {
            slot: FrameStorageSlotId::Local(slot),
            byte_offset: offset,
        },
        replay
            .constraints
            .keys
            .frame_address
            .ok_or_else(|| replay.invalid())?,
        &[register],
        &provenance,
    )?;
    Ok(register)
}
