//! Independently replay one shared view field's descriptor replacement: two
//! word loads from the established literal's descriptor, then two word stores
//! at the field's descriptor offset in the original root.
use super::{
    LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    SelectedInstructionKind, SelectedInstructionProvenance, SelectedMemoryAccessRole,
    StructuralAccess, memory,
};
use crate::SelectedInstructionError;
use crate::selection::validation::scalar_graph::Replay;
use crate::selection::validation::scalar_graph::structural::provenance;
use crate::selection::validation::scalar_graph::structural::result;

pub(super) fn replace(
    function: &LegalizedScalarFunction,
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::StructuralByteViewFieldStore {
        destination,
        field,
        source,
    } = &row.kind
    else {
        return Err(replay.invalid());
    };
    let signature = function
        .structural
        .as_ref()
        .ok_or_else(|| replay.invalid())?;
    let parameter = signature
        .parameters
        .iter()
        .find(|parameter| parameter.semantic.place == destination.place)
        .ok_or_else(|| replay.invalid())?;
    if row.result.is_some()
        || parameter.semantic.access != destination.access
        || !matches!(
            destination.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        )
        || parameter.semantic.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || !parameter.semantic.qualifications.is_empty()
        || !parameter.semantic.projected_qualifications.is_empty()
        || *source == destination.place
        || crate::selection::established_view_input::literal_view_type(function, *source).is_none()
    {
        return Err(replay.invalid());
    }
    let descriptor_offset =
        crate::structural_inputs::structural_reference_input::byte_view_field_descriptor(
            parameter.semantic.structural_type,
            &destination.path,
            *field,
            &signature.structural_types,
        )
        .ok_or_else(|| replay.invalid())?;
    let extent_offset = descriptor_offset
        .checked_add(8)
        .ok_or_else(|| replay.invalid())?;
    let literal = replay
        .transport
        .pointers
        .iter()
        .find(|(place, _)| place == source)
        .map(|(_, pointer)| *pointer)
        .ok_or_else(|| replay.invalid())?;
    let root = replay
        .transport
        .pointers
        .iter()
        .find(|(place, _)| *place == destination.place)
        .map(|(_, pointer)| *pointer)
        .ok_or_else(|| replay.invalid())?;
    let load64 = replay
        .constraints
        .keys
        .load64
        .ok_or_else(|| replay.invalid())?;
    let store = replay
        .constraints
        .keys
        .store
        .ok_or_else(|| replay.invalid())?;
    let backing = result(replay, *source, 0)?;
    memory(
        replay,
        row,
        *source,
        0,
        8,
        SelectedMemoryAccessRole::ReadPlace,
    )?;
    replay.check_instruction(
        SelectedInstructionKind::Load64 { byte_offset: 0 },
        load64,
        &[literal, backing],
        &provenance(row),
    )?;
    let extent = result(replay, *source, 8)?;
    memory(
        replay,
        row,
        *source,
        8,
        8,
        SelectedMemoryAccessRole::ReadPlace,
    )?;
    replay.check_instruction(
        SelectedInstructionKind::Load64 { byte_offset: 8 },
        load64,
        &[literal, extent],
        &provenance(row),
    )?;
    memory(
        replay,
        row,
        destination.place,
        descriptor_offset,
        8,
        SelectedMemoryAccessRole::WritePlace,
    )?;
    replay.check_instruction(
        SelectedInstructionKind::Store {
            byte_offset: descriptor_offset,
            byte_size: 8,
        },
        store,
        &[root, backing],
        &provenance(row),
    )?;
    memory(
        replay,
        row,
        destination.place,
        extent_offset,
        8,
        SelectedMemoryAccessRole::WritePlace,
    )?;
    replay.check_instruction(
        SelectedInstructionKind::Store {
            byte_offset: extent_offset,
            byte_size: 8,
        },
        store,
        &[root, extent],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )
}
