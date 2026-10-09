//! Materialize a reference's address word without copying referent storage.

use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    SelectedInstructionKind, SelectedMemoryAccessRole, invalid, memory, place_pointer, provenance,
    transport_register,
};
use crate::SelectedInstructionError;
use selected_instructions::{LocalStorageSlotId, SelectedLocalStorageSlot};

pub(super) fn establish(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let address = crate::selection::reference_input::establishment(source, row).ok_or(invalid())?;
    let LegalizedScalarInstructionKind::EstablishReference { result, .. } = &row.kind else {
        return Err(invalid());
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
    // Element projections borrow retained backing storage. A view's carrier
    // home is only appropriate when borrowing the whole descriptor itself.
    let projected_base = if address.projects_root_storage {
        super::runtime_address::projection_base(
            source,
            builder,
            row,
            address.place,
            address.byte_offset,
        )?
    } else {
        None
    };
    let mut input = match projected_base {
        Some(pointer) => pointer,
        // Inline ABI fragments have no registered address until this loan
        // requests their home; view payload resolution above never declines.
        None => place_pointer(builder, row, address.place, 0)?,
    };
    for (offset, indices) in address.descriptor_offsets {
        input = super::runtime_address::scale(
            builder,
            row,
            address.place,
            offset,
            input,
            &address.indices[indices],
        )?;
        let loaded = transport_register(builder, result.place, offset)?;
        memory(
            builder,
            row,
            address.place,
            offset,
            8,
            SelectedMemoryAccessRole::ReadPlace,
        )?;
        builder.emit(
            SelectedInstructionKind::Load64 {
                byte_offset: offset,
            },
            builder.constraints.keys.load64.ok_or(invalid())?,
            &[input, loaded],
            provenance(row),
        )?;
        input = loaded;
    }
    input = super::runtime_address::scale(
        builder,
        row,
        address.place,
        address.byte_offset,
        input,
        &address.indices[address.leaf_index..],
    )?;
    if address.byte_offset != 0 {
        let projected = transport_register(builder, result.place, address.byte_offset)?;
        builder.emit(
            SelectedInstructionKind::AddressOffset {
                byte_offset: address.byte_offset,
            },
            builder.constraints.keys.address_offset.ok_or(invalid())?,
            &[input, projected],
            provenance(row),
        )?;
        input = projected;
    }
    if let Some(storage) = address.storage {
        let (_, offset, _, scalar) = builder.resolve(storage.byte_offset).ok_or(invalid())?;
        if !matches!(scalar, semantic_vocabulary::ScalarType::Integer(integer) if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned && integer.bits() == 64)
        {
            return Err(invalid());
        }
        let projected = transport_register(builder, result.place, 0)?;
        builder.emit(
            SelectedInstructionKind::ByteViewAddress,
            builder.constraints.keys.add_i64,
            &[input, offset, projected],
            provenance(row),
        )?;
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
        builder.emit(
            SelectedInstructionKind::Load64 { byte_offset: 0 },
            builder.constraints.keys.load64.ok_or(invalid())?,
            &[input, value],
            provenance(row),
        )?;
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
        builder.emit(
            SelectedInstructionKind::Load64 { byte_offset: 8 },
            builder.constraints.keys.load64.ok_or(invalid())?,
            &[input, codec],
            provenance(row),
        )?;
        Some(codec)
    } else if let Some(codec) = address.codec {
        let register = transport_register(builder, result.place, 8)?;
        builder.emit(
            SelectedInstructionKind::MaterializeI64 {
                value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(
                    codec.encode().ok_or(invalid())?,
                )),
            },
            builder.constraints.keys.materialize_i64,
            &[register],
            provenance(row),
        )?;
        Some(register)
    } else {
        None
    };
    let byte_size = if codec.is_some() { 16 } else { 8 };
    let slot = LocalStorageSlotId::Structural {
        operation: row.operation,
        place: result.place,
    };
    builder
        .transport
        .local_slots
        .push(SelectedLocalStorageSlot {
            id: slot,
            byte_size,
            alignment: 8,
        });
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
        builder.emit(
            SelectedInstructionKind::Store {
                byte_offset: 8,
                byte_size: 8,
            },
            builder.constraints.keys.store.ok_or(invalid())?,
            &[home, codec],
            provenance(row),
        )?;
    }
    memory(
        builder,
        row,
        result.place,
        0,
        8,
        SelectedMemoryAccessRole::WritePlace,
    )?;
    builder.emit(
        SelectedInstructionKind::Store {
            byte_offset: 0,
            byte_size: 8,
        },
        builder.constraints.keys.store.ok_or(invalid())?,
        &[home, value],
        provenance(row),
    )?;
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
    builder.emit(
        SelectedInstructionKind::Load64 {
            byte_offset: displacement,
        },
        builder.constraints.keys.load64.ok_or(invalid())?,
        &[address, pointer],
        provenance,
    )?;
    Ok(pointer)
}

pub(in crate::selection::construction::scalar_graph) fn borrowed_argument(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: semantic_vocabulary::PlaceId,
    address: &crate::selection::reference_input::BorrowedReferenceAddress,
) -> Result<selected_instructions::VirtualRegisterId, SelectedInstructionError> {
    // Keep ordinary call ownership/provenance admission before loading payload.
    let mut pointer = super::place_pointer(builder, row, place, 0)?;
    let descriptor_count = address.descriptor_offsets.len();
    for (position, offset) in address.descriptor_offsets.iter().copied().enumerate() {
        let home = transport_register(builder, place, offset)?;
        builder.emit(
            if offset == 0 {
                SelectedInstructionKind::CopyI64
            } else {
                SelectedInstructionKind::AddressOffset {
                    byte_offset: offset,
                }
            },
            if offset == 0 {
                builder.constraints.keys.copy_i64
            } else {
                builder.constraints.keys.address_offset.ok_or(invalid())?
            },
            &[pointer, home],
            provenance(row),
        )?;
        if address.scalar_descriptor && position + 1 == descriptor_count {
            if address.referent_offset != 0 {
                return Err(invalid());
            }
            return Ok(home);
        }
        pointer = transport_register(builder, place, offset)?;
        memory(
            builder,
            row,
            place,
            offset,
            8,
            SelectedMemoryAccessRole::ReadPlace,
        )?;
        builder.emit(
            SelectedInstructionKind::Load64 { byte_offset: 0 },
            builder.constraints.keys.load64.ok_or(invalid())?,
            &[home, pointer],
            provenance(row),
        )?;
    }
    let offset = address.referent_offset;
    let output = transport_register(builder, place, offset)?;
    builder.emit(
        if offset == 0 {
            SelectedInstructionKind::CopyI64
        } else {
            SelectedInstructionKind::AddressOffset {
                byte_offset: offset,
            }
        },
        if offset == 0 {
            builder.constraints.keys.copy_i64
        } else {
            builder.constraints.keys.address_offset.ok_or(invalid())?
        },
        &[pointer, output],
        provenance(row),
    )?;
    Ok(output)
}
