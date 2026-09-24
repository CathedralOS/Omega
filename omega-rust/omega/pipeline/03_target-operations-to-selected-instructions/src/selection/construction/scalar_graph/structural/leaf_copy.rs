//! Copy one readable root's verified extent into fresh activation storage,
//! and reseat a borrowed window's vacated field with the inverse copy.
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
use selected_instructions::{LocalStorageSlotId, SelectedLocalStorageSlot, VirtualRegisterId};
use semantic_vocabulary::IntegerValue;
use semantic_vocabulary::PlaceId;

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
    copy_bytes(
        builder,
        row,
        CopyEnd {
            place: *source,
            pointer: input,
            byte_offset: *byte_offset,
        },
        CopyEnd {
            place: result.place,
            pointer,
            byte_offset: 0,
        },
        u32::from(shape.byte_size),
    )?;
    builder.transport.pointers.push((result.place, pointer));
    Ok(())
}

/// Reseat the field a borrowed-window move vacated: the inverse of `copy`.
/// The move copied the field out through the root's referent pointer; the
/// store reads the consumed value through its home's durable pointer and
/// writes the same extent back at the field's offset. Legalization replay has
/// already rejoined the offset and extent to the declared field and the
/// value's exact producer.
pub(super) fn reseat(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::StoreStructuralField {
        destination,
        value,
        byte_offset,
        shape,
        ..
    } = &row.kind
    else {
        return Err(invalid());
    };
    let signature = source.structural.as_ref().ok_or_else(|| invalid())?;
    if row.result.is_some()
        || destination.access != terminal_psi::StructuralAccess::MutableBorrow
        || !signature
            .parameters
            .iter()
            .any(|parameter| parameter.semantic == *destination)
    {
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
    let pointer_of = |place| {
        builder
            .transport
            .pointers
            .iter()
            .find(|(owner, _)| *owner == place)
            .map(|(_, pointer)| *pointer)
            .ok_or_else(|| invalid())
    };
    let root = pointer_of(destination.place)?;
    let home = pointer_of(*value)?;
    copy_bytes(
        builder,
        row,
        CopyEnd {
            place: *value,
            pointer: home,
            byte_offset: 0,
        },
        CopyEnd {
            place: destination.place,
            pointer: root,
            byte_offset: *byte_offset,
        },
        u32::from(shape.byte_size),
    )
}

/// One end of a chunked byte copy: the place whose storage is addressed, the
/// register holding a pointer to it, and the extent's offset from that
/// pointer.
struct CopyEnd {
    place: PlaceId,
    pointer: VirtualRegisterId,
    byte_offset: u32,
}

/// Copy `byte_size` bytes from `from` to `to` in the widest aligned-size
/// chunks: each chunk loads into a fresh transport register and stores it,
/// recording the read and write footprints on their places. Both directions
/// of a borrowed window and every leaf copy share this one routine.
fn copy_bytes(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    from: CopyEnd,
    to: CopyEnd,
    byte_size: u32,
) -> Result<(), SelectedInstructionError> {
    let mut cursor = 0u32;
    while cursor < byte_size {
        let width = chunk(byte_size - cursor);
        let load_offset = from.byte_offset + cursor;
        let value = transport_register(builder, from.place, load_offset)?;
        memory(
            builder,
            row,
            from.place,
            load_offset,
            u32::from(width),
            SelectedMemoryAccessRole::ReadPlace,
        )?;
        let (kind, key) = match width {
            8 => (
                SelectedInstructionKind::Load64 {
                    byte_offset: load_offset,
                },
                builder.constraints.keys.load64,
            ),
            4 => (
                SelectedInstructionKind::Load32 {
                    byte_offset: load_offset,
                },
                builder.constraints.keys.load32,
            ),
            2 => (
                SelectedInstructionKind::Load16 {
                    byte_offset: load_offset,
                },
                builder.constraints.keys.load16,
            ),
            _ => (
                SelectedInstructionKind::Load8 {
                    byte_offset: load_offset,
                },
                builder.constraints.keys.load8,
            ),
        };
        builder.emit(
            kind,
            key.ok_or_else(|| invalid())?,
            &[from.pointer, value],
            provenance(row),
        )?;
        let store_offset = to.byte_offset + cursor;
        memory(
            builder,
            row,
            to.place,
            store_offset,
            u32::from(width),
            SelectedMemoryAccessRole::WritePlace,
        )?;
        builder.emit(
            SelectedInstructionKind::Store {
                byte_offset: store_offset,
                byte_size: width,
            },
            builder.constraints.keys.store.ok_or_else(|| invalid())?,
            &[to.pointer, value],
            provenance(row),
        )?;
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
