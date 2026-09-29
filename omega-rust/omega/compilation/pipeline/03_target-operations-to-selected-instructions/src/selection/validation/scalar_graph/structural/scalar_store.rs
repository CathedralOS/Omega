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
                    destination.structural_type(),
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
            (
                destination.clone(),
                value,
                *byte_offset,
                *byte_size,
                indices,
            )
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
            (
                abstract_operations::StructuralScalarStoreDestination::Parameter(
                    destination.clone(),
                ),
                value,
                *byte_offset,
                *byte_size,
                indices,
            )
        }
        _ => return Err(replay.invalid()),
    };
    let signature = source.structural.as_ref().ok_or_else(|| replay.invalid())?;
    let declared = match &destination {
        abstract_operations::StructuralScalarStoreDestination::Parameter(parameter) => {
            (signature
                .parameters
                .iter()
                .any(|declared| declared.semantic == *parameter)
                || parameter.access == StructuralAccess::MutableBorrow
                    && crate::selection::address_join_input::join(source, parameter.place)
                        .is_some_and(|(_, declared)| declared == parameter))
                && matches!(
                    parameter.access,
                    StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                )
        }
        abstract_operations::StructuralScalarStoreDestination::OwnedResult { producer, result } => {
            crate::selection::aggregate_result_input::owned_store_destination(
                source, *producer, result,
            )
        }
    };
    if row.result.is_some() || !declared {
        return Err(replay.invalid());
    }
    let view_home = replay
        .transport
        .element_views
        .iter()
        .find(|view| view.place == destination.place())
        .copied();
    let destination_is_view = view_home.is_some()
        || crate::selection::established_view_input::element_view_type(source, destination.place())
            .is_some();
    let pointer = if destination_is_view {
        // The construction emits the descriptor load (or the home's backing
        // register) ahead of the store; replay checks the same sequence.
        let backing =
            super::element_views::element_backing_pointer(replay, row, destination.place())?;
        if let Some(view) = view_home {
            let base = super::result(replay, destination.place(), byte_offset)?;
            replay.check_instruction(
                SelectedInstructionKind::ByteViewAddress,
                replay.constraints.keys.add_i64,
                &[backing, view.byte_offset, base],
                &super::provenance(row),
            )?;
            base
        } else {
            backing
        }
    } else {
        replay
            .transport
            .pointers
            .iter()
            .find(|(place, _)| *place == destination.place())
            .map(|(_, register)| *register)
            .ok_or_else(|| replay.invalid())?
    };
    let (_, register, _, scalar) = replay
        .resolve(value.value)
        .ok_or_else(|| replay.invalid())?;
    if scalar != value.scalar_type {
        return Err(replay.invalid());
    }
    let address = super::runtime_address::scale(
        replay,
        row,
        destination.place(),
        byte_offset,
        pointer,
        indices,
    )?;
    let (address, displacement) = super::memory_pointer(
        replay,
        row,
        destination.place(),
        address,
        byte_offset,
        u32::from(byte_size),
    )?;
    super::runtime_address::footprint(
        replay,
        row,
        destination.place(),
        byte_offset,
        u32::from(byte_size),
        indices,
        super::runtime_address::Written::Scalar(value.value),
    )?;
    replay.check_instruction(
        SelectedInstructionKind::Store {
            byte_offset: displacement,
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
