//! Independent store replay from semantic declarations and SSA definitions.
use super::*;

pub(super) fn validate(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    replay: &mut Replay<'_>,
) -> Result<(), SelectedInstructionError> {
    let (destination, value, byte_offset, byte_size) = match &row.kind {
        LegalizedScalarInstructionKind::StructuralScalarFieldStore {
            destination,
            path,
            field,
            value,
            byte_offset,
            byte_size,
        } => {
            let signature = source.structural.as_ref().ok_or_else(|| replay.invalid())?;
            if crate::structural_reference_input::store(
                destination.structural_type,
                path,
                *field,
                value.scalar_type,
                &signature.structural_types,
            ) != Some((*byte_offset, *byte_size))
            {
                return Err(replay.invalid());
            }
            (destination, value, *byte_offset, *byte_size)
        }
        LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
            destination,
            value,
            byte_size,
        } => {
            let signature = source.structural.as_ref().ok_or_else(|| replay.invalid())?;
            if !signature.entry_claims.is_empty()
                || crate::structural_reference_input::primitive_store(
                    destination,
                    value.scalar_type,
                    &signature.structural_types,
                ) != Some(*byte_size)
            {
                return Err(replay.invalid());
            }
            (destination, value, 0, *byte_size)
        }
        _ => return Err(replay.invalid()),
    };
    let signature = source.structural.as_ref().ok_or_else(|| replay.invalid())?;
    if row.result.is_some()
        || !signature
            .parameters
            .iter()
            .any(|parameter| parameter.semantic == *destination)
        || !matches!(
            destination.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        )
    {
        return Err(replay.invalid());
    }
    let pointer = replay
        .transport
        .pointers
        .iter()
        .find(|(place, _)| *place == destination.place)
        .map(|(_, register)| *register)
        .ok_or_else(|| replay.invalid())?;
    let (_, register, _, scalar) = replay
        .resolve(value.value)
        .ok_or_else(|| replay.invalid())?;
    if scalar != value.scalar_type {
        return Err(replay.invalid());
    }
    memory(
        replay,
        row,
        destination.place,
        byte_offset,
        u32::from(byte_size),
        SelectedMemoryAccessRole::WritePlace,
    )?;
    replay.check_instruction(
        SelectedInstructionKind::Store {
            byte_offset,
            byte_size,
        },
        replay
            .constraints
            .keys
            .store
            .ok_or_else(|| replay.invalid())?,
        &[pointer, register],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![value.value],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(())
}
