//! Copy one readable root's verified leaf into fresh activation storage.
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    SelectedInstructionKind, SelectedMemoryAccessRole, memory,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::invalid;
use crate::selection::construction::scalar_graph::structural::local_storage;
use crate::selection::construction::scalar_graph::structural::provenance;
use crate::selection::construction::scalar_graph::structural::transport_register;
use selected_instructions::{LocalStorageSlotId, SelectedLocalStorageSlot};

/// The copy reads the root through its durable pointer — an entry-assigned
/// borrow, a retained owned home, a block arrival, or an earlier producer —
/// and never from ABI fragments, so an owned input reaching a leaf copy
/// without a pointer home fails closed. The result is published like an
/// established aggregate: slot first, chunked loads, chunked stores, then
/// the place's transport pointer.
pub(super) fn copy(
    _source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::StructuralLeafCopy {
        result,
        source,
        byte_offset,
        shape,
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
        .map(|(_, pointer)| *pointer)
        .ok_or_else(invalid)?;
    let slot = LocalStorageSlotId::Structural {
        operation: row.operation,
        place: result.place,
    };
    builder
        .transport
        .local_slots
        .push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: u32::from(shape.byte_size),
            alignment: shape.alignment,
        });
    let pointer = local_storage::address(builder, row, slot, 0, u32::from(shape.byte_size), true)?;
    let mut cursor = 0u32;
    while cursor < u32::from(shape.byte_size) {
        let width = chunk(u32::from(shape.byte_size) - cursor);
        let value = transport_register(builder, *source, *byte_offset + cursor)?;
        memory(
            builder,
            row,
            *source,
            *byte_offset + cursor,
            u32::from(width),
            SelectedMemoryAccessRole::ReadPlace,
        )?;
        let (kind, key) = match width {
            8 => (
                SelectedInstructionKind::Load64 {
                    byte_offset: *byte_offset + cursor,
                },
                builder.constraints.keys.load64,
            ),
            4 => (
                SelectedInstructionKind::Load32 {
                    byte_offset: *byte_offset + cursor,
                },
                builder.constraints.keys.load32,
            ),
            2 => (
                SelectedInstructionKind::Load16 {
                    byte_offset: *byte_offset + cursor,
                },
                builder.constraints.keys.load16,
            ),
            _ => (
                SelectedInstructionKind::Load8 {
                    byte_offset: *byte_offset + cursor,
                },
                builder.constraints.keys.load8,
            ),
        };
        builder.emit(
            kind,
            key.ok_or_else(invalid)?,
            &[input, value],
            provenance(row),
        )?;
        memory(
            builder,
            row,
            result.place,
            cursor,
            u32::from(width),
            SelectedMemoryAccessRole::WritePlace,
        )?;
        builder.emit(
            SelectedInstructionKind::Store {
                byte_offset: cursor,
                byte_size: width,
            },
            builder.constraints.keys.store.ok_or_else(invalid)?,
            &[pointer, value],
            provenance(row),
        )?;
        cursor += u32::from(width);
    }
    builder.transport.pointers.push((result.place, pointer));
    Ok(())
}

fn chunk(remaining: u32) -> u8 {
    [8u8, 4, 2, 1]
        .into_iter()
        .find(|width| u32::from(*width) <= remaining)
        .unwrap_or(1)
}
