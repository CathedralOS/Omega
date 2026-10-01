//! Establish a literal's addressable descriptor over its read-only image octets.
use super::local_storage::{address, store};
use super::{
    Builder, LegalizedScalarInstruction, LegalizedScalarInstructionKind, SelectedInstructionKind,
    VirtualRegisterId,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::invalid;
use crate::selection::construction::scalar_graph::structural::provenance;
use crate::selection::construction::scalar_graph::structural::transport_register;
use selected_instructions::{LocalStorageSlotId, SelectedLocalStorageSlot};
use semantic_vocabulary::{IntegerValue, StructuralPlaceKind};

pub(super) fn establish(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::EstablishByteSequenceLiteral {
        destination,
        structural_type,
        bytes,
    } = &row.kind
    else {
        return Err(invalid());
    };
    if row.result.is_some()
        || !matches!(destination.kind, StructuralPlaceKind::ByteSequenceLiteral { structural_type: identity, .. } if identity == structural_type.id)
        || !structural_type.shape.is_borrowed_byte_view()
        || builder
            .transport
            .pointers
            .iter()
            .any(|(place, _)| *place == destination.id)
    {
        return Err(invalid());
    }
    // The octets stay in read-only image storage, never in this frame: only
    // the two-word descriptor is activation-local, so a stored copy of it
    // outlives the activation that established it.
    let length = u64::try_from(bytes.len()).map_err(|_| invalid())?;
    let slot = LocalStorageSlotId::Structural {
        operation: row.operation,
        place: destination.id,
    };
    builder
        .transport
        .local_slots
        .push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: 16,
            alignment: 8,
        });
    let backing = transport_register(builder, destination.id, 16)?;
    builder.emit(
        SelectedInstructionKind::ByteLiteralImageAddress {
            operation: row.operation,
            place: destination.id,
        },
        builder
            .constraints
            .keys
            .byte_literal_image_address
            .ok_or_else(|| invalid())?,
        &[backing],
        provenance(row),
    )?;
    store(builder, row, slot, 0, backing)?;
    let length = constant(builder, row, 8, length)?;
    store(builder, row, slot, 8, length)?;
    let descriptor = address(builder, row, slot, 0, 16, true)?;
    builder
        .transport
        .pointers
        .push((destination.id, descriptor));
    Ok(())
}

fn constant(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    offset: u32,
    value: u64,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let LegalizedScalarInstructionKind::EstablishByteSequenceLiteral { destination, .. } =
        &row.kind
    else {
        return Err(invalid());
    };
    let register = transport_register(builder, destination.id, offset)?;
    builder.emit(
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(u128::from(value)),
        },
        builder.constraints.keys.materialize_i64,
        &[register],
        provenance(row),
    )?;
    Ok(register)
}
