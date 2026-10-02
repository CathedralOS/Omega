//! Replay the whole-home stores and activation-local home of a memberwise
//! structural local establishment.
use super::{LegalizedScalarFunction, LegalizedScalarInstruction, SelectedInstructionKind, result};
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
    let (established, shape) = crate::selection::structural_local_input::storage(source, row)
        .ok_or_else(|| SelectedInstructionError::custody())?;
    let slot = LocalStorageSlotId::Structural {
        operation: row.operation,
        place: established.place,
    };
    replay.transport.local_slots.push(SelectedLocalStorageSlot {
        id: slot,
        byte_size: shape.byte_size.max(1),
        alignment: shape.alignment,
    });
    // Match record construction's anchor without adding semantic payload.
    let pointer = local_storage::address(replay, row, slot, 0, shape.byte_size.max(1), true)?;
    if shape.byte_size == 0 {
        replay.transport.pointers.push((established.place, pointer));
        return Ok(());
    }
    // The member stores overwrite the members they establish; the home's
    // remaining bytes replay the same zero start construction emits.
    let zero = result(replay, established.place, 0)?;
    replay.check_instruction(
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(0),
        },
        replay.constraints.keys.materialize_i64,
        &[zero],
        &provenance(row),
    )?;
    let mut offset = 0u32;
    while offset < shape.byte_size {
        let width = super::record::chunk(shape.byte_size - offset, offset, offset);
        super::record::store(
            replay,
            row,
            established.place,
            pointer,
            offset,
            width,
            zero,
            Vec::new(),
            Vec::new(),
        )?;
        offset += u32::from(width);
    }
    // Edge transport can use this home only after replaying every store.
    replay.transport.pointers.push((established.place, pointer));
    Ok(())
}
