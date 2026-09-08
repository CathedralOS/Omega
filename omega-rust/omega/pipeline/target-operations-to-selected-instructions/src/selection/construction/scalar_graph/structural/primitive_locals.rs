//! Establish and access the original primitive referent in the activation frame.
use super::*;
use selected_instructions::{LocalStorageSlotId, SelectedLocalStorageSlot};

pub(super) fn write(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let (place, value, establishing) = match &row.kind {
        LegalizedScalarInstructionKind::EstablishPrimitiveLocal { result, value, .. } => {
            (result.place, *value, true)
        }
        LegalizedScalarInstructionKind::PrimitiveLocalStore { destination, value } => {
            (*destination, *value, false)
        }
        _ => return Err(invalid()),
    };
    let (producer, _, scalar, shape) =
        crate::selection::primitive_local_input::local(source, place).ok_or_else(invalid)?;
    if row.result.is_some() || scalar != value.scalar_type {
        return Err(invalid());
    }
    let pointer = if establishing {
        if producer != row.operation
            || builder
                .transport
                .pointers
                .iter()
                .any(|(stored, _)| *stored == place)
        {
            return Err(invalid());
        }
        let slot = LocalStorageSlotId::Structural {
            operation: producer,
            place,
        };
        builder
            .transport
            .local_slots
            .push(SelectedLocalStorageSlot {
                id: slot,
                byte_size: u32::from(shape.byte_size),
                alignment: shape.alignment,
            });
        let pointer =
            local_storage::address(builder, row, slot, 0, u32::from(shape.byte_size), false)?;
        builder.transport.pointers.push((place, pointer));
        pointer
    } else {
        builder
            .transport
            .pointers
            .iter()
            .find(|(stored, _)| *stored == place)
            .map(|(_, pointer)| *pointer)
            .ok_or_else(invalid)?
    };
    let (_, value_register, _, actual) = builder.resolve(value.value).ok_or_else(invalid)?;
    if actual != scalar {
        return Err(invalid());
    }
    let byte_size = u8::try_from(shape.byte_size).map_err(|_| invalid())?;
    memory(
        builder,
        row,
        place,
        0,
        u32::from(byte_size),
        SelectedMemoryAccessRole::WritePlace,
    )?;
    builder.emit(
        SelectedInstructionKind::Store {
            byte_offset: 0,
            byte_size,
        },
        builder.constraints.keys.store.ok_or_else(invalid)?,
        &[pointer, value_register],
        SelectedInstructionProvenance {
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
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let LegalizedScalarInstructionKind::PrimitiveScalarRead { source: place } = row.kind else {
        return Err(invalid());
    };
    let definition = row.result.ok_or_else(invalid)?;
    if !crate::selection::primitive_local_input::readable(source, place, definition.scalar_type) {
        return Err(invalid());
    }
    let pointer = builder
        .transport
        .pointers
        .iter()
        .find(|(stored, _)| *stored == place)
        .map(|(_, pointer)| *pointer)
        .ok_or_else(invalid)?;
    let output = builder.register(
        definition.value,
        definition.definition_site,
        definition.scalar_type,
    )?;
    memory(
        builder,
        row,
        place,
        0,
        8,
        SelectedMemoryAccessRole::ReadPlace,
    )?;
    builder.emit(
        SelectedInstructionKind::Load64 { byte_offset: 0 },
        builder.constraints.keys.load64.ok_or_else(invalid)?,
        &[pointer, output],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![definition.value],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(output)
}
