//! Independently replay local addresses, exact writes and fresh scalar observations.
use super::*;
use selected_instructions::{LocalStorageSlotId, SelectedLocalStorageSlot};

pub(super) fn write(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    replay: &mut Replay<'_>,
) -> Result<(), SelectedInstructionError> {
    let (place, value, establishing) = match &row.kind {
        LegalizedScalarInstructionKind::EstablishPrimitiveLocal { result, value, .. } => {
            (result.place, *value, true)
        }
        LegalizedScalarInstructionKind::PrimitiveLocalStore { destination, value } => {
            (*destination, *value, false)
        }
        _ => return Err(replay.invalid()),
    };
    let (producer, _, scalar, shape) =
        crate::selection::primitive_local_input::local(source, place)
            .ok_or_else(|| replay.invalid())?;
    if row.result.is_some() || scalar != value.scalar_type {
        return Err(replay.invalid());
    }
    let pointer = if establishing {
        if producer != row.operation
            || replay
                .transport
                .pointers
                .iter()
                .any(|(stored, _)| *stored == place)
        {
            return Err(replay.invalid());
        }
        let slot = LocalStorageSlotId::Structural {
            operation: producer,
            place,
        };
        replay.transport.local_slots.push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: u32::from(shape.byte_size),
            alignment: shape.alignment,
        });
        let pointer =
            local_storage::address(replay, row, slot, 0, u32::from(shape.byte_size), false)?;
        replay.transport.pointers.push((place, pointer));
        pointer
    } else {
        replay
            .transport
            .pointers
            .iter()
            .find(|(stored, _)| *stored == place)
            .map(|(_, pointer)| *pointer)
            .ok_or_else(|| replay.invalid())?
    };
    let (_, value_register, _, actual) = replay
        .resolve(value.value)
        .ok_or_else(|| replay.invalid())?;
    if actual != scalar {
        return Err(replay.invalid());
    }
    let byte_size = u8::try_from(shape.byte_size).map_err(|_| replay.invalid())?;
    memory(
        replay,
        row,
        place,
        0,
        u32::from(byte_size),
        SelectedMemoryAccessRole::WritePlace,
    )?;
    replay.check_instruction(
        SelectedInstructionKind::Store {
            byte_offset: 0,
            byte_size,
        },
        replay
            .constraints
            .keys
            .store
            .ok_or_else(|| replay.invalid())?,
        &[pointer, value_register],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![value.value],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(())
}
pub(in crate::selection) fn read(
    source: &LegalizedScalarFunction,
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let LegalizedScalarInstructionKind::PrimitiveScalarRead { source: place } = row.kind else {
        return Err(replay.invalid());
    };
    let definition = row.result.ok_or_else(|| replay.invalid())?;
    if !crate::selection::primitive_local_input::readable(source, place, definition.scalar_type) {
        return Err(replay.invalid());
    }
    let pointer = replay
        .transport
        .pointers
        .iter()
        .find(|(stored, _)| *stored == place)
        .map(|(_, pointer)| *pointer)
        .ok_or_else(|| replay.invalid())?;
    let output = replay.result_register(
        definition.value,
        definition.definition_site,
        definition.scalar_type,
    )?;
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
        &[pointer, output],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![definition.value],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(output)
}
