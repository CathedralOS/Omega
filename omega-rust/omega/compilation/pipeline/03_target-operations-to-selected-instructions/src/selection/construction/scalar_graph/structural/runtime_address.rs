//! The address model for a structural access through runtime-selected
//! elements, shared by leaf copies, case observations, primitive reads and
//! stores, and scalar field stores (`grid[i][j]`, `ents[i].pos.x`).
//!
//! Resolve the storage base before adding projection offsets: a view's home
//! holds a descriptor, not its elements. All projected consumers use this
//! distinction; whole-view transport still copies the descriptor itself.
//!
//! The access's static part is the row's `byte_offset`, which the load or
//! store applies itself. Each runtime element joins the place pointer in path
//! order: its selector is sign- or zero-extended to 64 bits, multiplied by
//! the array's element stride, and added to the running address. Every
//! selector is proven inside its array by the obligation its Terminal path
//! segment carries, and legalization joined that certificate to the element,
//! so `index * stride` stays inside the array span and the wrapping multiply
//! is exact for every reachable operand. Nothing here re-derives a bound: the
//! obligation rides on the address instruction's provenance and on the
//! access's footprint rows. The corpus audits the retained target geometry;
//! construction consumes it without a second compiler validation pass.
use super::{
    Builder, IntegerSign, LegalizedScalarInstruction, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedMemoryAccessRole, VirtualRegisterId, memory,
    transport_register,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::invalid;
use legalized_operations::LegalizedRuntimeIndexOperand;
use semantic_vocabulary::{IntegerValue, PlaceId, ValueId};

/// Resolve the storage observed by a projection, before its static and runtime
/// displacements. View homes describe a loan: load its backing pointer
/// and retain a derived view's offset, never index the descriptor itself.
/// A missing address denotes ABI value fragments, not an empty or invalid loan;
/// callers separately admit the operations that can consume those fragments.
pub(super) fn projection_base(
    source: &super::LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    byte_offset: u32,
) -> Result<Option<VirtualRegisterId>, SelectedInstructionError> {
    let byte_view = crate::selection::established_view_input::view_type(source, place).is_some();
    let view_offset = if byte_view {
        builder.transport.view(place).map(|view| view.byte_offset)
    } else {
        builder
            .transport
            .element_view(place)
            .map(|view| view.byte_offset)
    };
    if !byte_view
        && view_offset.is_none()
        && crate::selection::established_view_input::element_view_type(source, place).is_none()
    {
        return Ok(builder
            .transport
            .pointers
            .iter()
            .find(|(stored, _)| *stored == place)
            .map(|(_, pointer)| *pointer));
    }
    let backing = if byte_view {
        super::byte_views::backing_pointer(source, builder, row, place)?
    } else {
        super::element_views::element_backing_pointer(builder, row, place)?
    };
    let Some(view_offset) = view_offset else {
        return Ok(Some(backing));
    };
    let base = transport_register(builder, place, byte_offset)?;
    builder.emit_byte_view_address(backing, view_offset, base, super::provenance(row))?;
    Ok(Some(base))
}

/// Start a projected access at the coordinate origin of its pointer regions.
/// A root view may live as backing/offset SSA without a descriptor home. Its
/// transport resolves the first crossing; replaying that slot would read an
/// element as a pointer. Nested crossings still load descriptors in path order,
/// after the root view's offset. Whole-descriptor observations use their home
/// directly and do not enter this operation.
pub(super) fn projection_origin<'slots>(
    source: &super::LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    byte_offset: u32,
    mut slots: &'slots [crate::structural_inputs::structural_reference_input::PointerSlot],
) -> Result<
    (
        Option<VirtualRegisterId>,
        &'slots [crate::structural_inputs::structural_reference_input::PointerSlot],
    ),
    SelectedInstructionError,
> {
    let root_view = crate::selection::established_view_input::view_type(source, place).is_some()
        || crate::selection::established_view_input::element_view_type(source, place).is_some();
    let pointer = if root_view {
        if let Some(first) = slots.first() {
            if first.byte_offset != 0 || first.indices != (0..0) || first.scalar_descriptor {
                return Err(invalid());
            }
            slots = &slots[1..];
        }
        projection_base(source, builder, row, place, byte_offset)?
    } else if !slots.is_empty() {
        Some(super::place_pointer(builder, row, place, 0)?)
    } else {
        projection_base(source, builder, row, place, byte_offset)?
    };
    Ok((pointer, slots))
}

/// Join each runtime element's scaled selector to `address`, in path order,
/// and return the register the access addresses with its static offset.
pub(in crate::selection) fn scale(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    byte_offset: u32,
    address: VirtualRegisterId,
    indices: &[LegalizedRuntimeIndexOperand],
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    scale_at_site(
        builder,
        super::provenance(row),
        place,
        byte_offset,
        address,
        indices,
    )
}

/// Operations and edge-owned cleanup share the address calculation. Retain
/// the real site rather than inventing an operation for an edge's invocation.
pub(in crate::selection) fn scale_at_site(
    builder: &mut Builder<'_>,
    provenance: SelectedInstructionProvenance,
    place: PlaceId,
    byte_offset: u32,
    mut address: VirtualRegisterId,
    indices: &[LegalizedRuntimeIndexOperand],
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    for index in indices {
        let (_, index_register, _, index_type) = builder
            .resolve(index.operand.value)
            .ok_or_else(|| invalid())?;
        let Some(integer) = index.operand.scalar_type.as_integer() else {
            return Err(invalid());
        };
        if integer.bits() > 64 || index_type != index.operand.scalar_type {
            return Err(invalid());
        }
        // Narrow selectors join the 64-bit address model after the same
        // sign- or zero-normalization scalar transport applies elsewhere.
        let index_register = match integer.bits() {
            64 => index_register,
            bits => {
                let kind = match (integer.sign(), bits) {
                    (IntegerSign::Unsigned, 8) => SelectedInstructionKind::ZeroExtendU8,
                    (IntegerSign::Unsigned, 16) => SelectedInstructionKind::ZeroExtendU16,
                    (IntegerSign::Unsigned, 32) => SelectedInstructionKind::ZeroExtendU32,
                    (IntegerSign::Signed, 8) => SelectedInstructionKind::SignExtendI8,
                    (IntegerSign::Signed, 16) => SelectedInstructionKind::SignExtendI16,
                    (IntegerSign::Signed, 32) => SelectedInstructionKind::SignExtendI32,
                    _ => return Err(invalid()),
                };
                let extended = transport_register(builder, place, byte_offset)?;
                builder.emit(
                    kind,
                    builder.constraints.keys.copy_i64,
                    &[index_register, extended],
                    SelectedInstructionProvenance {
                        values: vec![index.operand.value],
                        ..provenance.clone()
                    },
                )?;
                extended
            }
        };
        let stride = transport_register(builder, place, byte_offset)?;
        builder.emit_materialize_i64(
            IntegerValue::Unsigned(u128::from(index.stride)),
            stride,
            provenance.clone(),
        )?;
        let scaled = transport_register(builder, place, byte_offset)?;
        builder.emit(
            SelectedInstructionKind::WrappingMultiplyI64,
            builder.constraints.keys.multiply_i64,
            &[index_register, stride, scaled],
            SelectedInstructionProvenance {
                values: vec![index.operand.value],
                ..provenance.clone()
            },
        )?;
        let joined = transport_register(builder, place, byte_offset)?;
        builder.emit_byte_view_address(
            address,
            scaled,
            joined,
            SelectedInstructionProvenance {
                values: vec![index.operand.value],
                obligations: vec![index.obligation],
                ..provenance.clone()
            },
        )?;
        address = joined;
    }
    Ok(address)
}

/// What a footprint's write stores: a named scalar `value` (a primitive or
/// scalar field store), a structural element byte copy (a borrowed window's
/// reseat — chunks carry no single semantic value), or nothing (a read).
pub(super) enum Written {
    None,
    Scalar(ValueId),
    Structural,
}

/// Record the footprint of the next access instruction: the exact place
/// extent for a static projection, or one row per runtime element, in path
/// order, whose certificate proves that element's selector inside its array.
/// A write names the stored value on each element row.
pub(super) fn footprint(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    byte_offset: u32,
    byte_count: u32,
    indices: &[LegalizedRuntimeIndexOperand],
    written: Written,
) -> Result<(), SelectedInstructionError> {
    if indices.is_empty() {
        let role = if matches!(written, Written::None) {
            SelectedMemoryAccessRole::ReadPlace
        } else {
            SelectedMemoryAccessRole::WritePlace
        };
        return memory(builder, row, place, byte_offset, byte_count, role);
    }
    for index in indices {
        let role = match written {
            Written::Scalar(value) => SelectedMemoryAccessRole::WriteIndexedPrimitive {
                index: index.operand.value,
                value,
                stride: index.stride,
                extent: index.extent,
                obligation: index.obligation,
                accepted_fact: index.accepted_fact,
            },
            Written::Structural => SelectedMemoryAccessRole::WriteIndexedStructural {
                index: index.operand.value,
                stride: index.stride,
                extent: index.extent,
                obligation: index.obligation,
                accepted_fact: index.accepted_fact,
            },
            Written::None => SelectedMemoryAccessRole::ReadIndexedPrimitive {
                index: index.operand.value,
                stride: index.stride,
                extent: index.extent,
                obligation: index.obligation,
                accepted_fact: index.accepted_fact,
            },
        };
        memory(builder, row, place, byte_offset, byte_count, role)?;
    }
    Ok(())
}

/// Evaluate captured pointer slots in path order before observing the final
/// region. Stores and reads use the same address operation, retaining each
/// pointer load and its footprint at the original storage boundary.
pub(in crate::selection) fn cross_references(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    mut pointer: VirtualRegisterId,
    indices: &[LegalizedRuntimeIndexOperand],
    pointer_slots: &[crate::structural_inputs::structural_reference_input::PointerSlot],
) -> Result<(VirtualRegisterId, usize), SelectedInstructionError> {
    // Every reference crossing ends its own contiguous address region. Preserve
    // each pointer load before applying the following region's offset or indices.
    let mut leaf_index = 0;
    for slot in pointer_slots {
        // Scalar endpoints require the codec-aware operation below. Never
        // discard a descriptor word and fall back to semantic-width access.
        if slot.scalar_descriptor {
            return Err(invalid());
        }
        let slot_indices = indices.get(slot.indices.clone()).ok_or(invalid())?;
        let address = scale(builder, row, place, slot.byte_offset, pointer, slot_indices)?;
        let (address, displacement) =
            super::memory_pointer(builder, row, place, address, slot.byte_offset, 8)?;
        footprint(
            builder,
            row,
            place,
            slot.byte_offset,
            8,
            slot_indices,
            Written::None,
        )?;
        let loaded = super::transport_register(builder, place, slot.byte_offset)?;
        builder.emit_load64(displacement, address, loaded, super::provenance(row))?;
        pointer = loaded;
        leaf_index = slot.indices.end;
    }
    Ok((pointer, leaf_index))
}

/// Load an internal scalar endpoint's address and codec together. Every
/// preceding aggregate crossing retains its original address-region order.
pub(super) fn scalar_reference(
    source: &super::LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    pointer: VirtualRegisterId,
    indices: &[LegalizedRuntimeIndexOperand],
    slots: &[crate::structural_inputs::structural_reference_input::PointerSlot],
) -> Result<Option<(VirtualRegisterId, VirtualRegisterId)>, SelectedInstructionError> {
    let (home, byte_offset, traversals) = if let Some(slot) =
        slots.last().filter(|slot| slot.scalar_descriptor)
    {
        let (pointer, _) = cross_references(
            builder,
            row,
            place,
            pointer,
            indices,
            &slots[..slots.len() - 1],
        )?;
        let traversals = indices.get(slot.indices.clone()).ok_or(invalid())?;
        let home = scale(builder, row, place, slot.byte_offset, pointer, traversals)?;
        (home, slot.byte_offset, traversals)
    } else if crate::selection::reference_input::borrowed_scalar_source(source, place).is_some() {
        (pointer, 0, &[][..])
    } else {
        return Ok(None);
    };
    let (home, displacement) = super::memory_pointer(builder, row, place, home, byte_offset, 16)?;
    let address = transport_register(builder, place, byte_offset)?;
    footprint(
        builder,
        row,
        place,
        byte_offset,
        8,
        traversals,
        Written::None,
    )?;
    builder.emit_load64(displacement, home, address, super::provenance(row))?;
    let codec_offset = byte_offset.checked_add(8).ok_or(invalid())?;
    let codec = transport_register(builder, place, codec_offset)?;
    footprint(
        builder,
        row,
        place,
        codec_offset,
        8,
        traversals,
        Written::None,
    )?;
    builder.emit_load64(
        displacement.checked_add(8).ok_or(invalid())?,
        home,
        codec,
        super::provenance(row),
    )?;
    Ok(Some((address, codec)))
}
