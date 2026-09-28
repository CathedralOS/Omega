//! Independently replay complete carrier initialization and exact payload stores.
use super::super::{IntegerValue, ValueId};
use super::{
    LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    SelectedInstructionKind, SelectedInstructionProvenance, SelectedMemoryAccessRole,
    VirtualRegisterId, memory,
};
use crate::SelectedInstructionError;
use crate::selection::validation::scalar_graph::Replay;
use crate::selection::validation::scalar_graph::structural::local_storage;
use crate::selection::validation::scalar_graph::structural::provenance;
use crate::selection::validation::scalar_graph::structural::result;
use selected_instructions::{LocalStorageSlotId, SelectedLocalStorageSlot};

pub(super) fn establish(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    replay: &mut Replay<'_>,
) -> Result<(), SelectedInstructionError> {
    let (ordinal, placements) = crate::selection::aggregate_result_input::fields(source, row)
        .ok_or_else(|| SelectedInstructionError::custody())?;
    let LegalizedScalarInstructionKind::EstablishCase {
        result: established,
        fields,
        layout,
        ..
    } = &row.kind
    else {
        return Err(SelectedInstructionError::custody());
    };
    let slot = LocalStorageSlotId::Structural {
        operation: row.operation,
        place: established.place,
    };
    replay.transport.local_slots.push(SelectedLocalStorageSlot {
        id: slot,
        byte_size: layout.shape.byte_size,
        alignment: layout.shape.alignment,
    });
    let pointer = local_storage::address(replay, row, slot, 0, layout.shape.byte_size, true)?;
    let zero = result(replay, established.place, 0)?;
    replay.check_instruction(
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(0),
        },
        replay.constraints.keys.materialize_i64,
        &[zero],
        &provenance(row),
    )?;
    let mut offset = 0;
    while offset < layout.shape.byte_size {
        let remaining = layout.shape.byte_size - offset;
        let width = [8, 4, 2, 1]
            .into_iter()
            .find(|width| *width <= remaining)
            .ok_or_else(|| SelectedInstructionError::custody())?;
        store(replay, row, pointer, offset, width as u8, zero, Vec::new())?;
        offset += width;
    }
    let tag = result(replay, established.place, 0)?;
    replay.check_instruction(
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(ordinal as u128),
        },
        replay.constraints.keys.materialize_i64,
        &[tag],
        &provenance(row),
    )?;
    store(replay, row, pointer, 0, 4, tag, Vec::new())?;
    super::record::write_fields(
        source,
        row,
        replay,
        established.place,
        pointer,
        fields,
        &placements,
    )?;
    replay.transport.pointers.push((established.place, pointer));
    Ok(())
}

fn store(
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
    pointer: VirtualRegisterId,
    offset: u32,
    width: u8,
    value: VirtualRegisterId,
    values: Vec<ValueId>,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::EstablishCase { result, .. } = &row.kind else {
        return Err(replay.invalid());
    };
    memory(
        replay,
        row,
        result.place,
        offset,
        u32::from(width),
        SelectedMemoryAccessRole::WritePlace,
    )?;
    replay.check_instruction(
        SelectedInstructionKind::Store {
            byte_offset: offset,
            byte_size: width,
        },
        replay
            .constraints
            .keys
            .store
            .ok_or_else(|| replay.invalid())?,
        &[pointer, value],
        &SelectedInstructionProvenance {
            values,
            ..provenance(row)
        },
    )
}
