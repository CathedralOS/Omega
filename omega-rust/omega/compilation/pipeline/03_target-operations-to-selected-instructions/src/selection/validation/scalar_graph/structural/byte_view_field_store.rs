//! Independently replay one shared view field's descriptor replacement: two
//! word loads from the established source's descriptor, then two word stores
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
        indices,
    } = &row.kind
    else {
        return Err(replay.invalid());
    };
    let signature = function
        .structural
        .as_ref()
        .ok_or_else(|| replay.invalid())?;
    let root_type = crate::selection::field_bytes::root_type(function, destination)
        .ok_or_else(|| replay.invalid())?;
    let geometry = crate::structural_inputs::structural_reference_input::projected_byte_field(
        root_type,
        &destination.path,
        *field,
        &signature.structural_types,
    )
    .ok_or_else(|| replay.invalid())?;
    if row.result.is_some()
        || !matches!(
            destination.access,
            StructuralAccess::Owned
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow
        )
        || *source == destination.place
        || !crate::selection::established_view_input::view_type(function, *source).is_some_and(
            |identity| {
                signature
                    .structural_types
                    .iter()
                    .any(|row| row.id == identity && row.shape.is_borrowed_byte_view())
            },
        )
        || !crate::structural_inputs::structural_reference_input::runtime_indices_match(
            indices,
            &geometry.elements,
        )
    {
        return Err(replay.invalid());
    }
    let descriptor_offset = geometry.descriptor_offset.ok_or_else(|| replay.invalid())?;
    let extent_offset = descriptor_offset
        .checked_add(8)
        .ok_or_else(|| replay.invalid())?;
    let descriptor = replay
        .transport
        .pointers
        .iter()
        .find(|(place, _)| place == source)
        .map(|(_, pointer)| *pointer)
        .ok_or_else(|| replay.invalid())?;
    let root = super::runtime_address::projection_base(
        function,
        replay,
        row,
        destination.place,
        descriptor_offset,
    )?
    .ok_or_else(|| replay.invalid())?;
    let root = super::runtime_address::scale(
        replay,
        row,
        destination.place,
        descriptor_offset,
        root,
        indices,
    )?;
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
        &[descriptor, backing],
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
        &[descriptor, extent],
        &provenance(row),
    )?;
    super::runtime_address::footprint(
        replay,
        row,
        destination.place,
        descriptor_offset,
        8,
        indices,
        super::runtime_address::Written::Structural,
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
    super::runtime_address::footprint(
        replay,
        row,
        destination.place,
        extent_offset,
        8,
        indices,
        super::runtime_address::Written::Structural,
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
