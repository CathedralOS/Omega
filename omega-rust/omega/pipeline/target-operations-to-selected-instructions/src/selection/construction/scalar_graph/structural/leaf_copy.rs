//! Copy one readable root's verified leaf into fresh activation storage.
use super::{
    Builder, IntegerSign, IntegerType, LegalizedScalarFunction, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind, ScalarType, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedMemoryAccessRole, memory,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::invalid;
use crate::selection::construction::scalar_graph::structural::local_storage;
use crate::selection::construction::scalar_graph::structural::provenance;
use crate::selection::construction::scalar_graph::structural::transport_register;
use selected_instructions::{LocalStorageSlotId, SelectedLocalStorageSlot};
use semantic_vocabulary::IntegerValue;

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
        index,
        index_stride,
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
    // A runtime index scales by the declared element stride and joins the
    // root's pointer ahead of the first load. The index is proven inside the
    // array extent, so `index * stride` lands inside the array span and the
    // wrapping multiply is exact for every reachable operand — the same
    // address model the indexed store emits.
    let input = if let Some(index) = index {
        let (_, index_register, _, index_type) =
            builder.resolve(index.value).ok_or_else(invalid)?;
        let unsigned_64 = IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?;
        if index.scalar_type != ScalarType::Integer(unsigned_64) || index_type != index.scalar_type
        {
            return Err(invalid());
        }
        let stride = transport_register(builder, *source, *byte_offset)?;
        builder.emit(
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(u128::from(*index_stride)),
            },
            builder.constraints.keys.materialize_i64,
            &[stride],
            SelectedInstructionProvenance {
                operations: vec![row.operation],
                ..Default::default()
            },
        )?;
        let scaled = transport_register(builder, *source, *byte_offset)?;
        builder.emit(
            SelectedInstructionKind::WrappingMultiplyI64,
            builder.constraints.keys.multiply_i64,
            &[index_register, stride, scaled],
            SelectedInstructionProvenance {
                operations: vec![row.operation],
                values: vec![index.value],
                ..Default::default()
            },
        )?;
        let address = transport_register(builder, *source, *byte_offset)?;
        builder.emit(
            SelectedInstructionKind::ByteViewAddress,
            builder.constraints.keys.add_i64,
            &[input, scaled, address],
            SelectedInstructionProvenance {
                operations: vec![row.operation],
                values: vec![index.value],
                ..Default::default()
            },
        )?;
        address
    } else {
        input
    };
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
