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
    let shape = crate::selection::scalar_call_abi::scalar_shape(definition.scalar_type)
        .ok_or_else(|| replay.invalid())?;
    let (instruction, constraint) = match shape.byte_size {
        1 => (
            SelectedInstructionKind::Load8 { byte_offset: 0 },
            replay.constraints.keys.load8,
        ),
        2 => (
            SelectedInstructionKind::Load16 { byte_offset: 0 },
            replay.constraints.keys.load16,
        ),
        4 => (
            SelectedInstructionKind::Load32 { byte_offset: 0 },
            replay.constraints.keys.load32,
        ),
        8 => (
            SelectedInstructionKind::Load64 { byte_offset: 0 },
            replay.constraints.keys.load64,
        ),
        _ => return Err(replay.invalid()),
    };
    memory(
        replay,
        row,
        place,
        0,
        u32::from(shape.byte_size),
        SelectedMemoryAccessRole::ReadPlace,
    )?;
    replay.check_instruction(
        instruction,
        constraint.ok_or_else(|| replay.invalid())?,
        &[pointer, output],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![definition.value],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    // The exact-width load zero-extends raw bits. Restore a narrow signed
    // value before any consumer, not only at a subsequent ABI boundary.
    // Only the memory load carries the authored read's operation and fuel.
    if matches!(definition.scalar_type, ScalarType::Integer(integer)
        if integer.sign() == IntegerSign::Signed && matches!(integer.bits(), 8 | 16 | 32))
    {
        let normalized = replay.result_register(
            definition.value,
            definition.definition_site,
            definition.scalar_type,
        )?;
        replay.check_instruction(
            crate::selection::scalar_call_abi::integer_abi_normalization(definition.scalar_type),
            replay.constraints.keys.copy_i64,
            &[output, normalized],
            &SelectedInstructionProvenance {
                values: vec![definition.value],
                ..Default::default()
            },
        )?;
        return Ok(normalized);
    }
    Ok(output)
}
