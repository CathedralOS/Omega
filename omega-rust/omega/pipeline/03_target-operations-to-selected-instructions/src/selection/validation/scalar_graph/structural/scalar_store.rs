//! Independent store replay from semantic declarations and SSA definitions.
//! A path through runtime-selected elements replays the address scaling
//! (`runtime_address`) before the store row.
use super::{
    LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    SelectedInstructionKind, SelectedInstructionProvenance, StructuralAccess,
};
use crate::SelectedInstructionError;
use crate::selection::validation::scalar_graph::Replay;
use crate::structural_inputs::structural_reference_input::runtime_indices_match;

pub(super) fn validate(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    replay: &mut Replay<'_>,
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
            let signature = source.structural.as_ref().ok_or_else(|| replay.invalid())?;
            let (offset, size, elements) =
                crate::structural_inputs::structural_reference_input::store(
                    destination.structural_type,
                    path,
                    *field,
                    value.scalar_type,
                    &signature.structural_types,
                )
                .ok_or_else(|| replay.invalid())?;
            if (offset, size) != (*byte_offset, *byte_size)
                || !runtime_indices_match(indices, &elements)
            {
                return Err(replay.invalid());
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
            let signature = source.structural.as_ref().ok_or_else(|| replay.invalid())?;
            let (offset, size, elements) =
                crate::structural_inputs::structural_reference_input::primitive_store(
                    destination,
                    path,
                    value.scalar_type,
                    &signature.structural_types,
                )
                .ok_or_else(|| replay.invalid())?;
            if !signature.entry_claims.is_empty()
                || (offset, size) != (*byte_offset, *byte_size)
                || !runtime_indices_match(indices, &elements)
            {
                return Err(replay.invalid());
            }
            (destination, value, *byte_offset, *byte_size, indices)
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
    let address = super::runtime_address::scale(
        replay,
        row,
        destination.place,
        byte_offset,
        pointer,
        indices,
    )?;
    super::runtime_address::footprint(
        replay,
        row,
        destination.place,
        byte_offset,
        u32::from(byte_size),
        indices,
        Some(value.value),
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
        &[address, register],
        &SelectedInstructionProvenance {
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
