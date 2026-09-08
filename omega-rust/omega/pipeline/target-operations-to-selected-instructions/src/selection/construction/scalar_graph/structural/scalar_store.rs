//! Exact non-observing writes through original borrowed pointers.
use super::*;

pub(super) fn emit(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
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
            let signature = source.structural.as_ref().ok_or_else(invalid)?;
            if crate::structural_reference_input::store(
                destination.structural_type,
                path,
                *field,
                value.scalar_type,
                &signature.structural_types,
            ) != Some((*byte_offset, *byte_size))
            {
                return Err(invalid());
            }
            (destination, value, *byte_offset, *byte_size)
        }
        LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
            destination,
            value,
            byte_size,
        } => {
            let signature = source.structural.as_ref().ok_or_else(invalid)?;
            if !signature.entry_claims.is_empty()
                || crate::structural_reference_input::primitive_store(
                    destination,
                    value.scalar_type,
                    &signature.structural_types,
                ) != Some(*byte_size)
            {
                return Err(invalid());
            }
            (destination, value, 0, *byte_size)
        }
        _ => return Err(invalid()),
    };
    let signature = source.structural.as_ref().ok_or_else(invalid)?;
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
        return Err(invalid());
    }
    let pointer = builder
        .transport
        .pointers
        .iter()
        .find(|(place, _)| *place == destination.place)
        .map(|(_, register)| *register)
        .ok_or_else(invalid)?;
    let (_, register, _, scalar) = builder.resolve(value.value).ok_or_else(invalid)?;
    if scalar != value.scalar_type {
        return Err(invalid());
    }
    memory(
        builder,
        row,
        destination.place,
        byte_offset,
        u32::from(byte_size),
        SelectedMemoryAccessRole::WritePlace,
    )?;
    builder.emit(
        SelectedInstructionKind::Store {
            byte_offset,
            byte_size,
        },
        builder.constraints.keys.store.ok_or_else(invalid)?,
        &[pointer, register],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![value.value],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(())
}
