//! Independently replay the original address and the carrier's one-word store.

use super::{
    LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    SelectedInstructionKind, SelectedMemoryAccessRole, memory, place_pointer, provenance, result,
};
use crate::SelectedInstructionError;
use crate::selection::validation::scalar_graph::Replay;
use selected_instructions::{LocalStorageSlotId, SelectedLocalStorageSlot};

pub(super) fn establish(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    replay: &mut Replay<'_>,
) -> Result<(), SelectedInstructionError> {
    let address = crate::selection::reference_input::establishment(source, row)
        .ok_or_else(|| replay.invalid())?;
    let LegalizedScalarInstructionKind::EstablishReference {
        result: declaration,
        ..
    } = &row.kind
    else {
        return Err(replay.invalid());
    };
    if replay
        .transport
        .pointers
        .iter()
        .any(|(place, _)| *place == declaration.place)
        || replay
            .transport
            .local_slots
            .iter()
            .any(|slot| slot.id.structural_place() == Some(declaration.place))
    {
        return Err(replay.invalid());
    }
    let input = place_pointer(replay, row, address.place, address.byte_offset)?;
    let value = if address.load_pointer {
        let value = result(replay, declaration.place, 0)?;
        memory(
            replay,
            row,
            address.place,
            address.byte_offset,
            8,
            SelectedMemoryAccessRole::ReadPlace,
        )?;
        replay.check_instruction(
            SelectedInstructionKind::Load64 { byte_offset: 0 },
            replay
                .constraints
                .keys
                .load64
                .ok_or_else(|| replay.invalid())?,
            &[input, value],
            &provenance(row),
        )?;
        value
    } else {
        input
    };
    let slot = LocalStorageSlotId::Structural {
        operation: row.operation,
        place: declaration.place,
    };
    replay.transport.local_slots.push(SelectedLocalStorageSlot {
        id: slot,
        byte_size: 8,
        alignment: 8,
    });
    let home = super::local_storage::address(replay, row, slot, 0, 8, true)?;
    memory(
        replay,
        row,
        declaration.place,
        0,
        8,
        SelectedMemoryAccessRole::WritePlace,
    )?;
    replay.check_instruction(
        SelectedInstructionKind::Store {
            byte_offset: 0,
            byte_size: 8,
        },
        replay
            .constraints
            .keys
            .store
            .ok_or_else(|| replay.invalid())?,
        &[home, value],
        &provenance(row),
    )?;
    replay.transport.pointers.push((declaration.place, home));
    Ok(())
}

pub(in crate::selection::validation::scalar_graph) fn load(
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
    place: semantic_vocabulary::PlaceId,
    home: selected_instructions::VirtualRegisterId,
) -> Result<selected_instructions::VirtualRegisterId, SelectedInstructionError> {
    let pointer = result(replay, place, 0)?;
    memory(
        replay,
        row,
        place,
        0,
        8,
        SelectedMemoryAccessRole::ReadPlace,
    )?;
    replay.check_instruction(
        SelectedInstructionKind::Load64 { byte_offset: 0 },
        replay
            .constraints
            .keys
            .load64
            .ok_or_else(|| replay.invalid())?,
        &[home, pointer],
        &provenance(row),
    )?;
    Ok(pointer)
}

pub(in crate::selection::validation::scalar_graph) fn borrowed_argument(
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
    place: semantic_vocabulary::PlaceId,
    address: crate::selection::reference_input::BorrowedReferenceAddress,
) -> Result<selected_instructions::VirtualRegisterId, SelectedInstructionError> {
    let home = super::call_pointer(replay, row, place, address.descriptor_offset)?;
    let pointer = result(replay, place, address.descriptor_offset)?;
    memory(
        replay,
        row,
        place,
        address.descriptor_offset,
        8,
        SelectedMemoryAccessRole::ReadPlace,
    )?;
    replay.check_instruction(
        SelectedInstructionKind::Load64 { byte_offset: 0 },
        replay
            .constraints
            .keys
            .load64
            .ok_or_else(|| replay.invalid())?,
        &[home, pointer],
        &provenance(row),
    )?;
    let offset = address.referent_offset;
    let output = result(replay, place, offset)?;
    replay.check_instruction(
        if offset == 0 {
            SelectedInstructionKind::CopyI64
        } else {
            SelectedInstructionKind::AddressOffset {
                byte_offset: offset,
            }
        },
        if offset == 0 {
            replay.constraints.keys.copy_i64
        } else {
            replay
                .constraints
                .keys
                .address_offset
                .ok_or_else(|| replay.invalid())?
        },
        &[pointer, output],
        &provenance(row),
    )?;
    Ok(output)
}
