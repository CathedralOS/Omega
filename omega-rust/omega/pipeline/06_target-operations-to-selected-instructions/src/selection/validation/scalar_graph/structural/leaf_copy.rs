//! Replay the durable-root extent copy into fresh activation storage and the
//! borrowed window's inverse reseat.
use super::{
    IntegerSign, LegalizedScalarFunction, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind, ScalarType, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedMemoryAccessRole, memory,
};
use crate::SelectedInstructionError;
use crate::selection::validation::scalar_graph::Replay;
use crate::selection::validation::scalar_graph::structural::local_storage;
use crate::selection::validation::scalar_graph::structural::provenance;
use selected_instructions::{LocalStorageSlotId, SelectedLocalStorageSlot, VirtualRegisterId};
use semantic_vocabulary::IntegerValue;
use semantic_vocabulary::PlaceId;

#[track_caller]
fn invalid() -> SelectedInstructionError {
    SelectedInstructionError::custody()
}

/// Mirror of construction `leaf_copy::copy`: the source resolves through its
/// durable pointer, or through ABI fragments for an owned parameter without
/// addressable storage, and the result publishes as a structural local slot
/// followed by chunked loads, chunked stores, and the place's transport
/// pointer.
pub(super) fn copy(
    _source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    replay: &mut Replay<'_>,
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
        replay.pending_provenance.operations.push(row.operation);
        replay
            .pending_provenance
            .fuel
            .extend(row.fuel.iter().cloned());
        return Ok(());
    }
    let input = replay
        .transport
        .pointers
        .iter()
        .find(|(place, _)| *place == *source)
        .map(|(_, pointer)| *pointer);
    let slot = LocalStorageSlotId::Structural {
        operation: row.operation,
        place: result.place,
    };
    replay.transport.local_slots.push(SelectedLocalStorageSlot {
        id: slot,
        byte_size: u32::from(shape.byte_size),
        alignment: shape.alignment,
    });
    let pointer = local_storage::address(replay, row, slot, 0, u32::from(shape.byte_size), true)?;
    let Some(mut input) = input else {
        check_fragments(
            _source,
            row,
            source,
            result.place,
            *byte_offset,
            *shape,
            pointer,
            indices,
            replay,
        )?;
        replay.transport.pointers.push((result.place, pointer));
        return Ok(());
    };
    for index in indices {
        let (_, index_register, _, index_type) = replay
            .resolve(index.operand.value)
            .ok_or_else(|| invalid())?;
        let ScalarType::Integer(integer) = index.operand.scalar_type else {
            return Err(invalid());
        };
        if integer.bits() > 64 || index_type != index.operand.scalar_type {
            return Err(invalid());
        }
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
                let extended = super::result(replay, *source, *byte_offset)?;
                replay.check_instruction(
                    kind,
                    replay.constraints.keys.copy_i64,
                    &[index_register, extended],
                    &SelectedInstructionProvenance {
                        operations: vec![row.operation],
                        values: vec![index.operand.value],
                        ..Default::default()
                    },
                )?;
                extended
            }
        };
        let stride = super::result(replay, *source, *byte_offset)?;
        replay.check_instruction(
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(u128::from(index.stride)),
            },
            replay.constraints.keys.materialize_i64,
            &[stride],
            &SelectedInstructionProvenance {
                operations: vec![row.operation],
                ..Default::default()
            },
        )?;
        let scaled = super::result(replay, *source, *byte_offset)?;
        replay.check_instruction(
            SelectedInstructionKind::WrappingMultiplyI64,
            replay.constraints.keys.multiply_i64,
            &[index_register, stride, scaled],
            &SelectedInstructionProvenance {
                operations: vec![row.operation],
                values: vec![index.operand.value],
                ..Default::default()
            },
        )?;
        let address = super::result(replay, *source, *byte_offset)?;
        replay.check_instruction(
            SelectedInstructionKind::ByteViewAddress,
            replay.constraints.keys.add_i64,
            &[input, scaled, address],
            &SelectedInstructionProvenance {
                operations: vec![row.operation],
                values: vec![index.operand.value],
                ..Default::default()
            },
        )?;
        input = address;
    }
    copy_bytes(
        replay,
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
    replay.transport.pointers.push((result.place, pointer));
    Ok(())
}

/// Replay mirror of construction `reseat`: the consumed value's home is read
/// through its durable pointer and written through the root's referent
/// pointer at the vacated field's offset, by the same chunked copy.
pub(super) fn reseat(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    replay: &mut Replay<'_>,
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
        replay.pending_provenance.operations.push(row.operation);
        replay
            .pending_provenance
            .fuel
            .extend(row.fuel.iter().cloned());
        return Ok(());
    }
    let pointer_of = |place| {
        replay
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
        replay,
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

/// One end of a replayed chunked byte copy; see construction `CopyEnd`.
struct CopyEnd {
    place: PlaceId,
    pointer: VirtualRegisterId,
    byte_offset: u32,
}

/// Replay mirror of construction `copy_bytes`.
fn copy_bytes(
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
    from: CopyEnd,
    to: CopyEnd,
    byte_size: u32,
) -> Result<(), SelectedInstructionError> {
    let mut cursor = 0u32;
    while cursor < byte_size {
        let width = chunk(byte_size - cursor);
        let load_offset = from.byte_offset + cursor;
        let value = super::result(replay, from.place, load_offset)?;
        memory(
            replay,
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
                replay.constraints.keys.load64,
            ),
            4 => (
                SelectedInstructionKind::Load32 {
                    byte_offset: load_offset,
                },
                replay.constraints.keys.load32,
            ),
            2 => (
                SelectedInstructionKind::Load16 {
                    byte_offset: load_offset,
                },
                replay.constraints.keys.load16,
            ),
            _ => (
                SelectedInstructionKind::Load8 {
                    byte_offset: load_offset,
                },
                replay.constraints.keys.load8,
            ),
        };
        replay.check_instruction(
            kind,
            key.ok_or_else(|| invalid())?,
            &[from.pointer, value],
            &provenance(row),
        )?;
        let store_offset = to.byte_offset + cursor;
        memory(
            replay,
            row,
            to.place,
            store_offset,
            u32::from(width),
            SelectedMemoryAccessRole::WritePlace,
        )?;
        replay.check_instruction(
            SelectedInstructionKind::Store {
                byte_offset: store_offset,
                byte_size: width,
            },
            replay.constraints.keys.store.ok_or_else(|| invalid())?,
            &[to.pointer, value],
            &provenance(row),
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

/// Replay mirror of construction `copy_from_fragments`: the same owned
/// parameter's inline ABI fragments store into the result slot at
/// leaf-relative offsets — a fragment cut by the leaf's bounds or any
/// runtime-index traversal stays honest-unsupported.
#[allow(clippy::too_many_arguments)]
fn check_fragments(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    place: &semantic_vocabulary::PlaceId,
    result_place: semantic_vocabulary::PlaceId,
    byte_offset: u32,
    shape: calling_conventions::ValueShape,
    pointer: selected_instructions::VirtualRegisterId,
    indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
    replay: &mut Replay<'_>,
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
        let value = replay
            .transport
            .fragments
            .iter()
            .find(|(stored, offset, _)| *stored == *place && *offset == fragment_offset)
            .map(|(_, _, value)| *value)
            .ok_or_else(|| invalid())?;
        let offset = fragment_offset - byte_offset;
        memory(
            replay,
            row,
            result_place,
            offset,
            u32::from(width),
            SelectedMemoryAccessRole::WritePlace,
        )?;
        replay.check_instruction(
            SelectedInstructionKind::Store {
                byte_offset: offset,
                byte_size: u8::try_from(width).map_err(|_| invalid())?,
            },
            replay.constraints.keys.store.ok_or_else(|| invalid())?,
            &[pointer, value],
            &provenance(row),
        )?;
    }
    Ok(())
}
