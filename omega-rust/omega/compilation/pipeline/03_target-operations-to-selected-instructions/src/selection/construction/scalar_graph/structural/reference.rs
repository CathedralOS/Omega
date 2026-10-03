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
    let address =
        crate::selection::reference_input::establishment(source, row).ok_or_else(invalid)?;
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
    let input = place_pointer(builder, row, address.place, address.byte_offset)?;
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
            builder.constraints.keys.load64.ok_or_else(invalid)?,
            &[input, value],
            provenance(row),
        )?;
        value
    } else {
        input
    };
    let slot = LocalStorageSlotId::Structural {
        operation: row.operation,
        place: result.place,
    };
    builder
        .transport
        .local_slots
        .push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: 8,
            alignment: 8,
        });
    let home = super::local_storage::address(builder, row, slot, 0, 8, true)?;
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
        builder.constraints.keys.store.ok_or_else(invalid)?,
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
    let pointer = transport_register(builder, place, 0)?;
    memory(
        builder,
        row,
        place,
        0,
        8,
        SelectedMemoryAccessRole::ReadPlace,
    )?;
    builder.emit(
        SelectedInstructionKind::Load64 { byte_offset: 0 },
        builder.constraints.keys.load64.ok_or_else(invalid)?,
        &[home, pointer],
        provenance(row),
    )?;
    Ok(pointer)
}

pub(in crate::selection::construction::scalar_graph) fn borrowed_argument(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: semantic_vocabulary::PlaceId,
    address: crate::selection::reference_input::BorrowedReferenceAddress,
) -> Result<selected_instructions::VirtualRegisterId, SelectedInstructionError> {
    // Keep ordinary call ownership/provenance admission before loading payload.
    let mut pointer = super::call_pointer(builder, row, place, 0)?;
    for offset in address.descriptor_offsets {
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
                builder
                    .constraints
                    .keys
                    .address_offset
                    .ok_or_else(invalid)?
            },
            &[pointer, home],
            provenance(row),
        )?;
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
            builder.constraints.keys.load64.ok_or_else(invalid)?,
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
            builder
                .constraints
                .keys
                .address_offset
                .ok_or_else(invalid)?
        },
        &[pointer, output],
        provenance(row),
    )?;
    Ok(output)
}
