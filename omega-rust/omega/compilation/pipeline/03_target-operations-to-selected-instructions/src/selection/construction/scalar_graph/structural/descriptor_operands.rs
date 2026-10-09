//! Read a selected descriptor in its original home. Pointer crossings and
//! runtime selectors keep their own ordered storage regions; this operation
//! neither extracts a descriptor nor grants an independent payload loan.
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, SelectedInstructionKind,
    SelectedMemoryAccessRole, VirtualRegisterId, invalid, memory, place_pointer, provenance,
    transport_register,
};
use crate::SelectedInstructionError;
use crate::structural_inputs::structural_reference_input::{
    self as geometry, ReferenceSourceGeometry,
};
use legalized_operations::LegalizedRuntimeIndexOperand;
use terminal_psi::{StructuralArgument, StructuralTypeShape};

pub(super) fn source_geometry(
    function: &LegalizedScalarFunction,
    source: &StructuralArgument,
) -> Option<(ReferenceSourceGeometry, StructuralTypeShape)> {
    let signature = function.structural.as_ref()?;
    let root = crate::selection::reference_input::root_type(function, source.place)?;
    let geometry =
        geometry::descriptor_source_geometry(root, &source.path, &signature.structural_types)?;
    if !geometry.readable || source.access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
        return None;
    }
    let shape = signature
        .structural_types
        .iter()
        .find(|declaration| declaration.id == geometry.selected)?
        .shape
        .clone();
    Some((geometry, shape))
}

pub(super) fn backing(
    function: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    source: &StructuralArgument,
    indices: &[LegalizedRuntimeIndexOperand],
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let (geometry, shape) = source_geometry(function, source).ok_or(invalid())?;
    if !geometry::runtime_indices_match(indices, &geometry.elements) {
        return Err(invalid());
    }
    let pointer = if source.path.is_empty() || !geometry.pointer_slots.is_empty() {
        place_pointer(builder, row, source.place, 0)?
    } else {
        super::runtime_address::projection_base(
            function,
            builder,
            row,
            source.place,
            geometry.byte_offset,
        )?
        .ok_or(invalid())?
    };
    let (pointer, leaf_index) = super::runtime_address::cross_references(
        builder,
        row,
        source.place,
        pointer,
        indices,
        &geometry.pointer_slots,
    )?;
    let leaf_indices = &indices[leaf_index..];
    let pointer = super::runtime_address::scale(
        builder,
        row,
        source.place,
        geometry.byte_offset,
        pointer,
        leaf_indices,
    )?;
    let (pointer, displacement) =
        super::memory_pointer(builder, row, source.place, pointer, geometry.byte_offset, 8)?;
    let output = transport_register(builder, source.place, geometry.byte_offset)?;
    match shape {
        StructuralTypeShape::ElementView { .. }
        | StructuralTypeShape::ByteSequence(
            terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
            | terminal_psi::ByteSequenceCarrier::BoundedView { .. },
        ) => {
            super::runtime_address::footprint(
                builder,
                row,
                source.place,
                geometry.byte_offset,
                8,
                leaf_indices,
                super::runtime_address::Written::None,
            )?;
            memory(
                builder,
                row,
                source.place,
                geometry.byte_offset,
                8,
                SelectedMemoryAccessRole::ReadPlace,
            )?;
            builder.emit(
                SelectedInstructionKind::Load64 {
                    byte_offset: displacement,
                },
                builder.constraints.keys.load64.ok_or(invalid())?,
                &[pointer, output],
                provenance(row),
            )?;
        }
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned {
            ..
        }) => {
            builder.emit(
                SelectedInstructionKind::AddressOffset {
                    byte_offset: displacement.checked_add(8).ok_or(invalid())?,
                },
                builder.constraints.keys.address_offset.ok_or(invalid())?,
                &[pointer, output],
                provenance(row),
            )?;
        }
        _ => return Err(invalid()),
    }
    Ok(output)
}

pub(super) fn element_stride(
    function: &LegalizedScalarFunction,
    source: &StructuralArgument,
) -> Option<u32> {
    let (_, StructuralTypeShape::ElementView { element }) = source_geometry(function, source)?
    else {
        return None;
    };
    let signature = function.structural.as_ref()?;
    let shape = geometry::shape(element, &signature.structural_types)?;
    geometry::align(shape.byte_size, shape.alignment).filter(|stride| *stride != 0)
}
