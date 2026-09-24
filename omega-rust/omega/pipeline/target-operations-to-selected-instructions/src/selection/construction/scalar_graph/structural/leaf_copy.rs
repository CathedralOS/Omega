//! Copy one readable root's verified leaf into fresh activation storage.
use super::{
    Builder, IntegerSign, LegalizedScalarFunction, LegalizedScalarInstruction,
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
/// or, for an owned parameter still carried as ABI fragments, through the
/// fragments themselves. The result is published like an established
/// aggregate: slot first, chunked loads, chunked stores, then the place's
/// transport pointer.
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
        indices,
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
    // An owned parameter can reach a leaf copy without addressable storage —
    // its payload lives in ABI fragments rather than behind a pointer.
    let Some(mut input) = input else {
        copy_from_fragments(
            _source,
            row,
            source,
            result.place,
            *byte_offset,
            *shape,
            pointer,
            indices,
            builder,
        )?;
        builder.transport.pointers.push((result.place, pointer));
        return Ok(());
    };
    // Each runtime index scales by its declared element stride and joins the
    // accumulating pointer in path order ahead of the first load. Every index
    // is proven inside its array extent, so `index * stride` lands inside the
    // array span and the wrapping multiply is exact for every reachable
    // operand — the same address model the indexed store emits.
    for index in indices {
        let (_, index_register, _, index_type) = builder
            .resolve(index.operand.value)
            .ok_or_else(|| invalid())?;
        let ScalarType::Integer(integer) = index.operand.scalar_type else {
            return Err(invalid());
        };
        if integer.bits() > 64 || index_type != index.operand.scalar_type {
            return Err(invalid());
        }
        // Narrow index operands join the 64-bit address model after the same
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
                let extended = transport_register(builder, *source, *byte_offset)?;
                builder.emit(
                    kind,
                    builder.constraints.keys.copy_i64,
                    &[index_register, extended],
                    SelectedInstructionProvenance {
                        operations: vec![row.operation],
                        values: vec![index.operand.value],
                        ..Default::default()
                    },
                )?;
                extended
            }
        };
        let stride = transport_register(builder, *source, *byte_offset)?;
        builder.emit(
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(u128::from(index.stride)),
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
                values: vec![index.operand.value],
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
                values: vec![index.operand.value],
                ..Default::default()
            },
        )?;
        input = address;
    }
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
            key.ok_or_else(|| invalid())?,
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
            builder.constraints.keys.store.ok_or_else(|| invalid())?,
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

/// Copy a leaf out of an owned parameter's inline ABI fragments: every
/// fragment fully inside the leaf's span stores its register into the result
/// slot at the leaf-relative offset — no loads, since the fragment already
/// holds the bytes. A fragment cut by the leaf's bounds would need a
/// sub-register read and stays honest-unsupported, as does any runtime-index
/// traversal (a data register cannot seed the address chain).
#[allow(clippy::too_many_arguments)]
fn copy_from_fragments(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    place: &semantic_vocabulary::PlaceId,
    result_place: semantic_vocabulary::PlaceId,
    byte_offset: u32,
    shape: calling_conventions::ValueShape,
    pointer: selected_instructions::VirtualRegisterId,
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
        .checked_add(u32::from(shape.byte_size))
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
            } => (u32::from(*value_byte_offset), *byte_size),
            _ => return Err(invalid()),
        };
        let fragment_end = fragment_offset
            .checked_add(u32::from(width))
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
        let offset = fragment_offset - byte_offset;
        memory(
            builder,
            row,
            result_place,
            offset,
            u32::from(width),
            SelectedMemoryAccessRole::WritePlace,
        )?;
        builder.emit(
            SelectedInstructionKind::Store {
                byte_offset: offset,
                byte_size: u8::try_from(width).map_err(|_| invalid())?,
            },
            builder.constraints.keys.store.ok_or_else(|| invalid())?,
            &[pointer, value],
            provenance(row),
        )?;
    }
    Ok(())
}
