//! Copy one readable root's verified extent into fresh activation storage,
//! and reseat a borrowed window's vacated field with the inverse copy.
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    SelectedInstructionKind,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::invalid;
use crate::selection::construction::scalar_graph::structural::local_storage;
use crate::selection::construction::scalar_graph::structural::provenance;
use crate::selection::construction::scalar_graph::structural::transport_register;
use selected_instructions::VirtualRegisterId;
use semantic_vocabulary::PlaceId;

/// The copy reads the root through its durable pointer — an entry-assigned
/// borrow, a retained owned home, a block arrival, or an earlier producer —
/// or, for an owned parameter still carried as ABI fragments, through the
/// fragments themselves. The result is published like an established
/// aggregate: slot first, chunked loads, chunked stores, then the place's
/// transport pointer.
pub(super) fn copy(
    function: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::StructuralLeafCopy {
        result,
        source,
        byte_offset,
        shape,
        indices,
        path,
        ..
    } = &row.kind
    else {
        return Err(invalid());
    };
    if shape.byte_size == 0 {
        builder.pending_provenance.operations.push(row.operation);
        builder
            .pending_provenance
            .fuel
            .extend(row.fuel.iter().cloned());
        return Ok(());
    }
    let input = builder
        .transport
        .pointers
        .iter()
        .find(|(place, _)| *place == *source)
        .map(|(_, pointer)| *pointer);
    let slot = builder.transport.push_structural_local_slot(
        row.operation,
        result.place,
        shape.byte_size,
        shape.alignment,
    );
    let pointer = local_storage::address(builder, row, slot, 0, shape.byte_size, true)?;
    let geometry = match path {
        abstract_operations::StructuralCopyPath::Ordinary(path) if !path.is_empty() => {
            let signature = function.structural.as_ref().ok_or(invalid())?;
            let root =
                crate::selection::reference_input::root_type(function, *source).ok_or(invalid())?;
            Some(
                crate::structural_inputs::structural_reference_input::leaf_copy_geometry(
                    root,
                    path,
                    result.structural_type,
                    &signature.structural_types,
                )
                .filter(|geometry| geometry.readable)
                .ok_or(invalid())?,
            )
        }
        _ => None,
    };
    let pointer_slots = geometry
        .as_ref()
        .map_or(&[][..], |geometry| geometry.pointer_slots.as_slice());
    let (input, pointer_slots) = if geometry.is_some() {
        super::runtime_address::projection_origin(
            function,
            builder,
            row,
            *source,
            *byte_offset,
            pointer_slots,
        )?
    } else {
        // Whole-value copies retain descriptor bytes, not backing elements.
        (input, pointer_slots)
    };
    let Some(input) = input else {
        copy_from_fragments(
            function,
            row,
            source,
            CopyEnd {
                place: result.place,
                pointer,
                byte_offset: 0,
                elements: &[],
            },
            *byte_offset,
            *shape,
            indices,
            builder,
        )?;
        builder.transport.pointers.push((result.place, pointer));
        return Ok(());
    };
    // Each runtime element scales its selector by the element stride and
    // joins the accumulating pointer in path order ahead of the first load
    // (`runtime_address`), the address model every runtime-projected access
    // shares.
    let (input, leaf_index) = super::runtime_address::cross_references(
        builder,
        row,
        *source,
        input,
        indices,
        pointer_slots,
    )?;
    let indices = indices.get(leaf_index..).ok_or(invalid())?;
    let input = super::runtime_address::scale(builder, row, *source, *byte_offset, input, indices)?;
    copy_bytes(
        builder,
        row,
        CopyEnd {
            place: *source,
            pointer: input,
            byte_offset: *byte_offset,
            elements: indices,
        },
        CopyEnd {
            place: result.place,
            pointer,
            byte_offset: 0,
            elements: &[],
        },
        shape.byte_size,
    )?;
    builder.transport.pointers.push((result.place, pointer));
    Ok(())
}

/// Reseat the field a borrowed-window move vacated: the inverse of `copy`.
/// The move copied the field out through the root's referent pointer, or a
/// view element out of the view's backing; the store reads the consumed value
/// through its durable pointer or incoming ABI fragments and writes the same
/// extent back at the same projection base. Legalization replay has already
/// rejoined the offset and extent to the declared field and the value's exact
/// producer.
pub(super) fn reseat(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::StoreStructuralField {
        destination,
        path,
        field,
        value,
        byte_offset,
        shape,
        indices,
        ..
    } = &row.kind
    else {
        return Err(invalid());
    };
    let destination_admits = match destination {
        abstract_operations::StructuralWindowRoot::Parameter(destination) => {
            matches!(
                destination.access,
                terminal_psi::StructuralAccess::Owned
                    | terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            ) && (source.structural.as_ref().is_some_and(|signature| {
                signature
                    .parameters
                    .iter()
                    .any(|parameter| parameter.semantic == *destination)
            }) || source
                .blocks
                .iter()
                .any(|block| block.structural_parameters.contains(destination)))
        }
        // An owned completed home's authority is its live transport pointer,
        // resolved against `destination.place` below.
        abstract_operations::StructuralWindowRoot::OwnedResult { .. } => true,
        abstract_operations::StructuralWindowRoot::OwnedBlockParameter { block, parameter } => {
            crate::selection::aggregate_result_input::block_home(source, parameter.place)
                == Some((*block, parameter))
        }
    };
    if row.result.is_some() || !destination_admits {
        return Err(invalid());
    }
    if shape.byte_size == 0 {
        builder.pending_provenance.operations.push(row.operation);
        builder
            .pending_provenance
            .fuel
            .extend(row.fuel.iter().cloned());
        return Ok(());
    }
    // Reuse the ordinary projection's ordered pointer regions. A descriptor
    // field is addressed in its containing home before loading its referent;
    // later offsets and selectors are relative to that loaded pointer.
    let types = &source
        .structural
        .as_ref()
        .ok_or(invalid())?
        .structural_types;
    let projection = terminal_semantics::structural_window_projection(
        types.iter(),
        destination.structural_type(),
        path,
        *field,
    )
    .ok_or(invalid())?;
    let geometry = crate::structural_inputs::structural_reference_input::reference_source_geometry(
        destination.structural_type(),
        &projection.path,
        types,
    )
    .ok_or(invalid())?;
    if geometry.byte_offset != *byte_offset
        || !crate::structural_inputs::structural_reference_input::runtime_indices_match(
            indices,
            &geometry.elements,
        )
    {
        return Err(invalid());
    }
    let (root, pointer_slots) = super::runtime_address::projection_origin(
        source,
        builder,
        row,
        destination.place(),
        *byte_offset,
        &geometry.pointer_slots,
    )?;
    let (root, leaf_index) = super::runtime_address::cross_references(
        builder,
        row,
        destination.place(),
        root.ok_or_else(invalid)?,
        indices,
        pointer_slots,
    )?;
    let indices = indices.get(leaf_index..).ok_or_else(invalid)?;
    let root = super::runtime_address::scale(
        builder,
        row,
        destination.place(),
        *byte_offset,
        root,
        indices,
    )?;
    let destination = CopyEnd {
        place: destination.place(),
        pointer: root,
        byte_offset: *byte_offset,
        elements: indices,
    };
    let home = builder
        .transport
        .pointers
        .iter()
        .find(|(place, _)| *place == *value)
        .map(|(_, pointer)| *pointer);
    let Some(home) = home else {
        return copy_from_fragments(source, row, value, destination, 0, *shape, &[], builder);
    };
    copy_bytes(
        builder,
        row,
        CopyEnd {
            place: *value,
            pointer: home,
            byte_offset: 0,
            elements: &[],
        },
        destination,
        shape.byte_size,
    )
}

/// One end of a chunked byte copy: the place whose storage is addressed, the
/// register holding a pointer to it, and the extent's offset from that
/// pointer.
struct CopyEnd<'a> {
    place: PlaceId,
    pointer: VirtualRegisterId,
    byte_offset: u32,
    /// The runtime elements the pointer already scaled in; a read through
    /// them publishes one element row per chunk rather than a place extent
    /// at the static offset, which the load does not address.
    elements: &'a [legalized_operations::LegalizedRuntimeIndexOperand],
}

/// Copy `byte_size` bytes from `from` to `to` in the widest aligned-size
/// chunks: each chunk loads into a fresh transport register and stores it,
/// recording the read and write footprints on their places. Both directions
/// of a borrowed window and every leaf copy share this one routine.
fn copy_bytes(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    from: CopyEnd<'_>,
    to: CopyEnd<'_>,
    byte_size: u32,
) -> Result<(), SelectedInstructionError> {
    let mut cursor = 0u32;
    while cursor < byte_size {
        let width = chunk(byte_size - cursor);
        let load_offset = from.byte_offset + cursor;
        let (input, load_displacement) = super::memory_pointer(
            builder,
            row,
            from.place,
            from.pointer,
            load_offset,
            u32::from(width),
        )?;
        let value = transport_register(builder, from.place, load_offset)?;
        super::runtime_address::footprint(
            builder,
            row,
            from.place,
            load_offset,
            u32::from(width),
            from.elements,
            super::runtime_address::Written::None,
        )?;
        let (kind, key) = match width {
            8 => (
                SelectedInstructionKind::Load64 {
                    byte_offset: load_displacement,
                },
                builder.constraints.keys.load64,
            ),
            4 => (
                SelectedInstructionKind::Load32 {
                    byte_offset: load_displacement,
                },
                builder.constraints.keys.load32,
            ),
            2 => (
                SelectedInstructionKind::Load16 {
                    byte_offset: load_displacement,
                },
                builder.constraints.keys.load16,
            ),
            _ => (
                SelectedInstructionKind::Load8 {
                    byte_offset: load_displacement,
                },
                builder.constraints.keys.load8,
            ),
        };
        builder.emit(
            kind,
            key.ok_or_else(|| invalid())?,
            &[input, value],
            provenance(row),
        )?;
        let store_offset = to.byte_offset + cursor;
        let (output, store_displacement) = super::memory_pointer(
            builder,
            row,
            to.place,
            to.pointer,
            store_offset,
            u32::from(width),
        )?;
        super::runtime_address::footprint(
            builder,
            row,
            to.place,
            store_offset,
            u32::from(width),
            to.elements,
            super::runtime_address::Written::Structural,
        )?;
        builder.emit_store(store_displacement, width, output, value, provenance(row))?;
        cursor += u32::from(width);
    }
    Ok(())
}

fn chunk(remaining: u32) -> u8 {
    [8u8, 4, 2, 1]
        .into_iter()
        .find(|width| u32::from(*width) <= remaining)
        .unwrap_or(1)
}

/// Copy a leaf out of an owned parameter's inline ABI fragments: every
/// fragment fully inside the leaf's span stores its register into the destination
/// at the leaf-relative offset — no loads, since the fragment already
/// holds the bytes. A fragment cut by the leaf's bounds would need a
/// sub-register read and stays honest-unsupported, as does any runtime-index
/// source traversal (a data register cannot seed the address chain). The
/// destination may be an indexed window; its pointer is already scaled and
/// each store retains the destination's original extent and index evidence.
#[allow(clippy::too_many_arguments)]
fn copy_from_fragments(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    place: &semantic_vocabulary::PlaceId,
    destination: CopyEnd<'_>,
    byte_offset: u32,
    shape: calling_conventions::ValueShape,
    indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    if !indices.is_empty() {
        return Err(invalid());
    }
    let parameter = source
        .structural
        .as_ref()
        .and_then(|signature| {
            signature
                .parameters
                .iter()
                .find(|parameter| parameter.semantic.place == *place)
        })
        .ok_or_else(|| invalid())?;
    if !crate::selection::aggregate_result_input::inline_argument_fragments(
        &parameter.target.placement,
    ) {
        return Err(invalid());
    }
    let leaf_end = byte_offset
        .checked_add(shape.byte_size)
        .ok_or_else(|| invalid())?;
    for location in &parameter.target.placement.locations {
        let (fragment_offset, width) = match location {
            calling_conventions::ValueLocation::Register {
                value_byte_offset,
                byte_size,
                ..
            }
            | calling_conventions::ValueLocation::Stack {
                value_byte_offset,
                byte_size,
                ..
            } => ((*value_byte_offset), *byte_size),
            _ => return Err(invalid()),
        };
        let fragment_end = fragment_offset
            .checked_add(width)
            .ok_or_else(|| invalid())?;
        if fragment_end <= byte_offset || fragment_offset >= leaf_end {
            continue;
        }
        if fragment_offset < byte_offset || fragment_end > leaf_end {
            return Err(invalid());
        }
        let value = builder
            .transport
            .fragments
            .iter()
            .find(|(stored, offset, _)| *stored == *place && *offset == fragment_offset)
            .map(|(_, _, value)| *value)
            .ok_or_else(|| invalid())?;
        let offset = destination
            .byte_offset
            .checked_add(fragment_offset - byte_offset)
            .ok_or_else(|| invalid())?;
        let (pointer, displacement) = super::memory_pointer(
            builder,
            row,
            destination.place,
            destination.pointer,
            offset,
            width,
        )?;
        super::runtime_address::footprint(
            builder,
            row,
            destination.place,
            offset,
            width,
            destination.elements,
            super::runtime_address::Written::Structural,
        )?;
        builder.emit_store(
            displacement,
            u8::try_from(width).map_err(|_| invalid())?,
            pointer,
            value,
            provenance(row),
        )?;
    }
    Ok(())
}
