//! Replay the durable-root leaf copy into fresh activation storage.
use super::{
    IntegerSign, IntegerType, LegalizedScalarFunction, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind, ScalarType, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedMemoryAccessRole, memory,
};
use crate::SelectedInstructionError;
use crate::selection::validation::scalar_graph::Replay;
use crate::selection::validation::scalar_graph::structural::local_storage;
use crate::selection::validation::scalar_graph::structural::provenance;
use selected_instructions::{LocalStorageSlotId, SelectedLocalStorageSlot};
use semantic_vocabulary::IntegerValue;

fn invalid() -> SelectedInstructionError {
    SelectedInstructionError::SourceCustodyMismatch
}

/// Mirror of construction `leaf_copy::copy`: the source resolves through its
/// durable pointer only, never ABI fragments, and the result publishes as a
/// structural local slot followed by chunked loads, chunked stores, and the
/// place's transport pointer.
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
        index,
        index_stride,
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
        .map(|(_, pointer)| *pointer)
        .ok_or_else(invalid)?;
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
    let input = if let Some(index) = index {
        let (_, index_register, _, index_type) = replay.resolve(index.value).ok_or_else(invalid)?;
        let unsigned_64 = IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?;
        if index.scalar_type != ScalarType::Integer(unsigned_64) || index_type != index.scalar_type
        {
            return Err(invalid());
        }
        let stride = super::result(replay, *source, *byte_offset)?;
        replay.check_instruction(
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(u128::from(*index_stride)),
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
                values: vec![index.value],
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
        let value = super::result(replay, *source, *byte_offset + cursor)?;
        memory(
            replay,
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
                replay.constraints.keys.load64,
            ),
            4 => (
                SelectedInstructionKind::Load32 {
                    byte_offset: *byte_offset + cursor,
                },
                replay.constraints.keys.load32,
            ),
            2 => (
                SelectedInstructionKind::Load16 {
                    byte_offset: *byte_offset + cursor,
                },
                replay.constraints.keys.load16,
            ),
            _ => (
                SelectedInstructionKind::Load8 {
                    byte_offset: *byte_offset + cursor,
                },
                replay.constraints.keys.load8,
            ),
        };
        replay.check_instruction(
            kind,
            key.ok_or_else(invalid)?,
            &[input, value],
            &provenance(row),
        )?;
        memory(
            replay,
            row,
            result.place,
            cursor,
            u32::from(width),
            SelectedMemoryAccessRole::WritePlace,
        )?;
        replay.check_instruction(
            SelectedInstructionKind::Store {
                byte_offset: cursor,
                byte_size: width,
            },
            replay.constraints.keys.store.ok_or_else(invalid)?,
            &[pointer, value],
            &provenance(row),
        )?;
        cursor += u32::from(width);
    }
    replay.transport.pointers.push((result.place, pointer));
    Ok(())
}

fn chunk(remaining: u32) -> u8 {
    [8u8, 4, 2, 1]
        .into_iter()
        .find(|width| u32::from(*width) <= remaining)
        .unwrap_or(1)
}
