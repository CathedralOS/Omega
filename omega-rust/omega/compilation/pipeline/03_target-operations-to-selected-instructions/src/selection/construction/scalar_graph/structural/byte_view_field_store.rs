//! Replace one shared view field's two-word descriptor with an established
//! source's: its backing pointer, then its extent. The source's addressable
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
        indices,
    } = &row.kind
    else {
        return Err(invalid());
    };
    let signature = function.structural.as_ref().ok_or_else(invalid)?;
    let root_type =
        crate::selection::field_bytes::root_type(function, destination).ok_or_else(|| invalid())?;
    let geometry = crate::structural_inputs::structural_reference_input::projected_byte_field(
        root_type,
        &destination.path,
        *field,
        &signature.structural_types,
    )
    .ok_or_else(|| invalid())?;
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
        return Err(invalid());
    }
    let descriptor_offset = geometry.descriptor_offset.ok_or_else(|| invalid())?;
    let extent_offset = descriptor_offset.checked_add(8).ok_or_else(invalid)?;
    let descriptor = builder
        .transport
        .pointers
        .iter()
        .find(|(place, _)| place == source)
        .map(|(_, pointer)| *pointer)
        .ok_or_else(invalid)?;
    let root = super::runtime_address::projection_base(
        function,
        builder,
        row,
        destination.place,
        descriptor_offset,
    )?
    .ok_or_else(|| invalid())?;
    let root = super::runtime_address::scale(
        builder,
        row,
        destination.place,
        descriptor_offset,
        root,
        indices,
    )?;
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
        &[descriptor, backing],
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
        &[descriptor, extent],
        provenance(row),
    )?;
    super::runtime_address::footprint(
        builder,
        row,
        destination.place,
        descriptor_offset,
        8,
        indices,
        super::runtime_address::Written::Structural,
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
    super::runtime_address::footprint(
        builder,
        row,
        destination.place,
        extent_offset,
        8,
        indices,
        super::runtime_address::Written::Structural,
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
