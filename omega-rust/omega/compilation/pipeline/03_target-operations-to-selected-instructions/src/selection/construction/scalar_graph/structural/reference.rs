//! Materialize a reference's address word without copying referent storage.

use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    SelectedMemoryAccessRole, invalid, memory, place_pointer, provenance, transport_at_byte_offset,
    transport_register,
};
use crate::SelectedInstructionError;

pub(super) fn establish(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let address = crate::selection::reference_input::establishment(source, row).ok_or(invalid())?;
    let result = match &row.kind {
        LegalizedScalarInstructionKind::EstablishReference { result, .. }
        | LegalizedScalarInstructionKind::EstablishPlacedReference { result, .. } => result,
        _ => return Err(invalid()),
    };
    let placed_allocation = if let Some(correspondence) = &address.placed {
        // A total factory cannot hide a failing alignment test or a trap.
        // Only an actual allocated home supplies this proof; pointer transport
        // and the logical source declaration do not authenticate its base.
        let mut allocations = builder
            .transport
            .local_slots
            .iter()
            .filter(|slot| slot.id.structural_place() == Some(address.place));
        let (Some(allocation), None) = (allocations.next(), allocations.next()) else {
            return Err(invalid());
        };
        let modulus = correspondence.required_base.modulus();
        if address.load_pointer
            || !address.pointer_slots.is_empty()
            || u64::from(allocation.alignment) % modulus != 0
            || u64::from(address.byte_offset) % modulus != correspondence.required_base.residue()
            || address
                .indices
                .iter()
                .any(|index| u64::from(index.stride) % modulus != 0)
        {
            return Err(invalid());
        }
        let maximum_offset = address
            .indices
            .iter()
            .try_fold(u64::from(address.byte_offset), |offset, index| {
                let last = index.extent?.checked_sub(1)?;
                offset.checked_add(last.checked_mul(u64::from(index.stride))?)
            })
            .ok_or_else(invalid)?;
        if maximum_offset
            .checked_add(correspondence.source_byte_length)
            .is_none_or(|end| end > u64::from(allocation.byte_size))
        {
            return Err(invalid());
        }
        Some(allocation.id)
    } else {
        None
    };
    if builder
        .transport
        .pointers
        .iter()
        .any(|(place, _)| *place == result.place)
        || builder
            .transport
            .local_slots
            .iter()
            .any(|slot| slot.id.structural_place() == Some(result.place))
    {
        return Err(invalid());
    }
    // Borrowing a descriptor retains its home. Only a declared pointer crossing
    // enters its backing; root carrier shape alone cannot choose that address.
    let (input, slots) = if let Some(allocation) = placed_allocation {
        // Emit the proved allocation's address directly. A cached pointer for
        // this place cannot select a different origin from the geometry proof.
        (
            super::local_storage::address(
                builder,
                row,
                allocation,
                0,
                u32::try_from(
                    address
                        .placed
                        .as_ref()
                        .ok_or_else(invalid)?
                        .source_byte_length,
                )
                .map_err(|_| invalid())?,
                false,
            )?,
            address.pointer_slots.as_slice(),
        )
    } else if address.pointer_slots.is_empty() {
        (
            place_pointer(builder, row, address.place, 0)?,
            address.pointer_slots.as_slice(),
        )
    } else {
        let (input, slots) = super::runtime_address::projection_origin(
            source,
            builder,
            row,
            address.place,
            address.byte_offset,
            &address.pointer_slots,
        )?;
        (input.ok_or_else(invalid)?, slots)
    };
    let (mut input, leaf_index) = super::runtime_address::cross_references(
        builder,
        row,
        address.place,
        input,
        &address.indices,
        slots,
    )?;
    input = super::runtime_address::scale(
        builder,
        row,
        address.place,
        address.byte_offset,
        input,
        &address.indices[leaf_index..],
    )?;
    if address.byte_offset != 0 {
        let projected = transport_register(builder, result.place, address.byte_offset)?;
        builder.emit_address_offset(address.byte_offset, input, projected, provenance(row))?;
        input = projected;
    }
    if let Some(storage) = address.storage {
        let (_, offset, _, scalar) = builder.resolve(storage.byte_offset).ok_or(invalid())?;
        if !matches!(scalar, semantic_vocabulary::ScalarType::Integer(integer) if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned && integer.bits() == 64)
        {
            return Err(invalid());
        }
        let projected = transport_register(builder, result.place, 0)?;
        builder.emit_byte_view_address(input, offset, projected, provenance(row))?;
        input = projected;
    }
    let value = if address.load_pointer {
        let value = transport_register(builder, result.place, 0)?;
        memory(
            builder,
            row,
            address.place,
            address.byte_offset,
            8,
            SelectedMemoryAccessRole::ReadPlace,
        )?;
        builder.emit_load64(0, input, value, provenance(row))?;
        value
    } else {
        input
    };
    let codec = if address.load_codec {
        let codec = transport_register(builder, result.place, 8)?;
        memory(
            builder,
            row,
            address.place,
            address.byte_offset.checked_add(8).ok_or(invalid())?,
            8,
            SelectedMemoryAccessRole::ReadPlace,
        )?;
        builder.emit_load64(8, input, codec, provenance(row))?;
        Some(codec)
    } else if let Some(codec) = address.codec {
        let register = transport_register(builder, result.place, 8)?;
        builder.emit_materialize_i64(
            semantic_vocabulary::IntegerValue::Unsigned(u128::from(
                codec.encode().ok_or(invalid())?,
            )),
            register,
            provenance(row),
        )?;
        Some(register)
    } else {
        None
    };
    let byte_size = if codec.is_some() { 16 } else { 8 };
    let slot =
        builder
            .transport
            .push_structural_local_slot(row.operation, result.place, byte_size, 8);
    let home = super::local_storage::address(builder, row, slot, 0, byte_size, true)?;
    if let Some(codec) = codec {
        memory(
            builder,
            row,
            result.place,
            8,
            8,
            SelectedMemoryAccessRole::WritePlace,
        )?;
        builder.emit_store(8, 8, home, codec, provenance(row))?;
    }
    memory(
        builder,
        row,
        result.place,
        0,
        8,
        SelectedMemoryAccessRole::WritePlace,
    )?;
    builder.emit_store(0, 8, home, value, provenance(row))?;
    // This table always addresses value storage. Consumers dereferencing the
    // reference must load the pointer word, not borrow this home as its referent.
    builder.transport.pointers.push((result.place, home));
    Ok(())
}

/// Read the selected address from a carrier home. The home address remains
/// registered as value storage; this transient pointer names referent storage.
pub(in crate::selection::construction::scalar_graph) fn load(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: semantic_vocabulary::PlaceId,
    home: selected_instructions::VirtualRegisterId,
) -> Result<selected_instructions::VirtualRegisterId, SelectedInstructionError> {
    load_pointer_word(
        builder,
        place,
        home,
        0,
        selected_instructions::SelectedMemoryAccessOrigin::Operation(row.operation),
        provenance(row),
    )
}

/// Read one descriptor word with its actual semantic owner. Terminator-owned
/// observations have block footprints, never an invented producer operation.
pub(in crate::selection::construction::scalar_graph) fn load_pointer_word(
    builder: &mut Builder<'_>,
    place: semantic_vocabulary::PlaceId,
    home: selected_instructions::VirtualRegisterId,
    byte_offset: u32,
    origin: selected_instructions::SelectedMemoryAccessOrigin,
    provenance: selected_instructions::SelectedInstructionProvenance,
) -> Result<selected_instructions::VirtualRegisterId, SelectedInstructionError> {
    let (address, displacement) =
        super::displaced_memory_pointer(builder, place, home, byte_offset, 8, provenance.clone())?;
    let pointer = transport_register(builder, place, byte_offset)?;
    builder
        .transport
        .memory
        .push(selected_instructions::SelectedMemoryAccess {
            instruction: selected_instructions::SelectedInstructionId(
                builder
                    .instructions
                    .len()
                    .try_into()
                    .map_err(|_| invalid())?,
            ),
            origin,
            place,
            byte_offset,
            byte_count: 8,
            role: SelectedMemoryAccessRole::ReadPlace,
        });
    builder.emit_load64(displacement, address, pointer, provenance)?;
    Ok(pointer)
}

/// Export the address word of an already-positioned scalar descriptor. Its
/// instruction displacement is zero, but retained memory custody names the
/// original slot and selectors. Reapplying that slot's offset would load a
/// different word; recording zero would attribute a padded field to its parent.
pub(in crate::selection::construction::scalar_graph) fn load_scalar_referent(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: semantic_vocabulary::PlaceId,
    descriptor: selected_instructions::VirtualRegisterId,
    reference: Option<&crate::selection::reference_input::BorrowedReferenceAddress>,
) -> Result<selected_instructions::VirtualRegisterId, SelectedInstructionError> {
    let (byte_offset, indices) = match reference {
        Some(reference) => {
            let slot = reference
                .pointer_slots
                .last()
                .filter(|slot| slot.scalar_descriptor)
                .ok_or_else(invalid)?;
            (
                slot.byte_offset,
                reference
                    .indices
                    .get(slot.indices.clone())
                    .ok_or_else(invalid)?,
            )
        }
        None => (0, &[][..]),
    };
    let pointer = transport_register(builder, place, byte_offset)?;
    super::runtime_address::footprint(
        builder,
        row,
        place,
        byte_offset,
        8,
        indices,
        super::runtime_address::Written::None,
    )?;
    builder.emit_load64(0, descriptor, pointer, provenance(row))?;
    Ok(pointer)
}

pub(in crate::selection::construction::scalar_graph) fn borrowed_argument(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: semantic_vocabulary::PlaceId,
    address: &crate::selection::reference_input::BorrowedReferenceAddress,
) -> Result<selected_instructions::VirtualRegisterId, SelectedInstructionError> {
    // The captured selectors stay in their original regions around pointer
    // loads. Scalar endpoints transport the descriptor home (address + codec),
    // while aggregate crossings load the stored address word.
    let pointer = super::place_pointer(builder, row, place, 0)?;
    let (slots, scalar_slot) = match address.pointer_slots.split_last() {
        Some((last, preceding)) if last.scalar_descriptor => (preceding, Some(last)),
        _ => (address.pointer_slots.as_slice(), None),
    };
    let (pointer, leaf_index) = super::runtime_address::cross_references(
        builder,
        row,
        place,
        pointer,
        &address.indices,
        slots,
    )?;
    let (pointer, offset) = if let Some(slot) = scalar_slot {
        if address.referent_offset != 0 || slot.indices.end != address.indices.len() {
            return Err(invalid());
        }
        (
            super::runtime_address::scale(
                builder,
                row,
                place,
                slot.byte_offset,
                pointer,
                address.indices.get(slot.indices.clone()).ok_or(invalid())?,
            )?,
            slot.byte_offset,
        )
    } else {
        (
            super::runtime_address::scale(
                builder,
                row,
                place,
                address.referent_offset,
                pointer,
                address.indices.get(leaf_index..).ok_or(invalid())?,
            )?,
            address.referent_offset,
        )
    };
    let output = transport_register(builder, place, offset)?;
    transport_at_byte_offset(builder, pointer, output, offset, provenance(row))?;
    Ok(output)
}
