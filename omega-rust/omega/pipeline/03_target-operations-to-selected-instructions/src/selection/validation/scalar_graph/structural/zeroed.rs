//! Replay the leaf stores and activation-local home of a zeroed establishment.
use super::{
    LegalizedScalarFunction, LegalizedScalarInstruction, SelectedInstructionKind,
    SelectedMemoryAccessRole, memory, result,
};
use crate::SelectedInstructionError;
use crate::selection::validation::scalar_graph::Replay;
use crate::selection::validation::scalar_graph::structural::local_storage;
use crate::selection::validation::scalar_graph::structural::provenance;
use selected_instructions::{LocalStorageSlotId, SelectedLocalStorageSlot};
use semantic_vocabulary::IntegerValue;

pub(super) fn establish(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    replay: &mut Replay<'_>,
) -> Result<(), SelectedInstructionError> {
    let (established, shape, stores) =
        crate::selection::zeroed_structural_input::storage(source, row)
            .ok_or_else(|| SelectedInstructionError::custody())?;
    if shape.byte_size == 0 {
        // The semantic result remains in the exact source frontier. No address,
        // physical home or access exists; retain construction before the next
        // executable instruction, including the block's terminator.
        replay.pending_provenance.operations.push(row.operation);
        replay
            .pending_provenance
            .fuel
            .extend(row.fuel.iter().cloned());
        return Ok(());
    }
    let slot = LocalStorageSlotId::Structural {
        operation: row.operation,
        place: established.place,
    };
    replay.transport.local_slots.push(SelectedLocalStorageSlot {
        id: slot,
        byte_size: u32::from(shape.byte_size),
        alignment: shape.alignment,
    });
    let pointer = local_storage::address(replay, row, slot, 0, u32::from(shape.byte_size), true)?;
    // Every scalar zero spelling is the all-zero bit pattern: construction
    // emits exactly one materialization shared by every leaf store.
    let zero = result(replay, established.place, 0)?;
    replay.check_instruction(
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(0),
        },
        replay.constraints.keys.materialize_i64,
        &[zero],
        &provenance(row),
    )?;
    for (offset, width) in stores {
        memory(
            replay,
            row,
            established.place,
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
            &[pointer, zero],
            &provenance(row),
        )?;
    }
    // Edge transport can use this home only after replaying every leaf store.
    replay.transport.pointers.push((established.place, pointer));
    Ok(())
}
