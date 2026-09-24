//! Exact non-observing writes through original borrowed pointers. A path
//! through runtime-selected elements scales each selector into the address
//! first (`runtime_address`); the store then applies the static offset.
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    SelectedInstructionKind, SelectedInstructionProvenance, StructuralAccess,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::invalid;
use crate::structural_inputs::structural_reference_input::runtime_indices_match;

pub(super) fn emit(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let (destination, value, byte_offset, byte_size, indices) = match &row.kind {
        LegalizedScalarInstructionKind::StructuralScalarFieldStore {
            destination,
            path,
            field,
            value,
            byte_offset,
            byte_size,
            indices,
        } => {
            let signature = source.structural.as_ref().ok_or_else(|| invalid())?;
            let (offset, size, elements) =
                crate::structural_inputs::structural_reference_input::store(
                    destination.structural_type,
                    path,
                    *field,
                    value.scalar_type,
                    &signature.structural_types,
                )
                .ok_or_else(|| invalid())?;
            if (offset, size) != (*byte_offset, *byte_size)
                || !runtime_indices_match(indices, &elements)
            {
                return Err(invalid());
            }
            (destination, value, *byte_offset, *byte_size, indices)
        }
        LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
            destination,
            path,
            value,
            byte_offset,
            byte_size,
            indices,
        } => {
            let signature = source.structural.as_ref().ok_or_else(|| invalid())?;
            let (offset, size, elements) =
                crate::structural_inputs::structural_reference_input::primitive_store(
                    destination,
                    path,
                    value.scalar_type,
                    &signature.structural_types,
                )
                .ok_or_else(|| invalid())?;
            if !signature.entry_claims.is_empty()
                || (offset, size) != (*byte_offset, *byte_size)
                || !runtime_indices_match(indices, &elements)
            {
                return Err(invalid());
            }
            (destination, value, *byte_offset, *byte_size, indices)
        }
        _ => return Err(invalid()),
    };
    let signature = source.structural.as_ref().ok_or_else(|| invalid())?;
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
        .ok_or_else(|| invalid())?;
    let (_, register, _, scalar) = builder.resolve(value.value).ok_or_else(|| invalid())?;
    if scalar != value.scalar_type {
        return Err(invalid());
    }
    let address = super::runtime_address::scale(
        builder,
        row,
        destination.place,
        byte_offset,
        pointer,
        indices,
    )?;
    super::runtime_address::footprint(
        builder,
        row,
        destination.place,
        byte_offset,
        u32::from(byte_size),
        indices,
        Some(value.value),
    )?;
    builder.emit(
        SelectedInstructionKind::Store {
            byte_offset,
            byte_size,
        },
        builder.constraints.keys.store.ok_or_else(|| invalid())?,
        &[address, register],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: indices
                .iter()
                .map(|index| index.operand.value)
                .chain([value.value])
                .collect(),
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(())
}
