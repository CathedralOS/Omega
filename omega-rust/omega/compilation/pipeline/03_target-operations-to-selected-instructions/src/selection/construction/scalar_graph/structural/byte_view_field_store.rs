//! Replace one shared view field's two-word descriptor with an established
//! literal's: its backing pointer, then its extent. The literal's addressable
//! descriptor is read word by word and stored at the field's descriptor
//! offset in the original root; no backing byte is read or copied.
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    SelectedInstructionKind, SelectedInstructionProvenance, SelectedMemoryAccessRole,
    StructuralAccess, memory,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::invalid;
use crate::selection::construction::scalar_graph::structural::provenance;
use crate::selection::construction::scalar_graph::structural::transport_register;

pub(super) fn replace(
    function: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::StructuralByteViewFieldStore {
        destination,
        field,
        source,
    } = &row.kind
    else {
        return Err(invalid());
    };
    let signature = function.structural.as_ref().ok_or_else(invalid)?;
    let parameter = signature
        .parameters
        .iter()
        .find(|parameter| parameter.semantic.place == destination.place)
        .ok_or_else(invalid)?;
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
        return Err(invalid());
    }
    let descriptor_offset =
        crate::structural_inputs::structural_reference_input::byte_view_field_descriptor(
            parameter.semantic.structural_type,
            &destination.path,
            *field,
            &signature.structural_types,
        )
        .ok_or_else(invalid)?;
    let extent_offset = descriptor_offset.checked_add(8).ok_or_else(invalid)?;
    let literal = builder
        .transport
        .pointers
        .iter()
        .find(|(place, _)| place == source)
        .map(|(_, pointer)| *pointer)
        .ok_or_else(invalid)?;
    let root = builder
        .transport
        .pointers
        .iter()
        .find(|(place, _)| *place == destination.place)
        .map(|(_, pointer)| *pointer)
        .ok_or_else(invalid)?;
    let backing = transport_register(builder, *source, 0)?;
    memory(
        builder,
        row,
        *source,
        0,
        8,
        SelectedMemoryAccessRole::ReadPlace,
    )?;
    builder.emit(
        SelectedInstructionKind::Load64 { byte_offset: 0 },
        builder.constraints.keys.load64.ok_or_else(invalid)?,
        &[literal, backing],
        provenance(row),
    )?;
    let extent = transport_register(builder, *source, 8)?;
    memory(
        builder,
        row,
        *source,
        8,
        8,
        SelectedMemoryAccessRole::ReadPlace,
    )?;
    builder.emit(
        SelectedInstructionKind::Load64 { byte_offset: 8 },
        builder.constraints.keys.load64.ok_or_else(invalid)?,
        &[literal, extent],
        provenance(row),
    )?;
    memory(
        builder,
        row,
        destination.place,
        descriptor_offset,
        8,
        SelectedMemoryAccessRole::WritePlace,
    )?;
    builder.emit(
        SelectedInstructionKind::Store {
            byte_offset: descriptor_offset,
            byte_size: 8,
        },
        builder.constraints.keys.store.ok_or_else(invalid)?,
        &[root, backing],
        provenance(row),
    )?;
    // The extent word commits the replacement and carries the fuel charge.
    memory(
        builder,
        row,
        destination.place,
        extent_offset,
        8,
        SelectedMemoryAccessRole::WritePlace,
    )?;
    builder.emit(
        SelectedInstructionKind::Store {
            byte_offset: extent_offset,
            byte_size: 8,
        },
        builder.constraints.keys.store.ok_or_else(invalid)?,
        &[root, extent],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )
}
