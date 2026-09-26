//! Establish a whole zero-constructible structural value in one home: a
//! single zero register stores into every scalar-leaf range.
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, SelectedInstructionKind,
    SelectedMemoryAccessRole, memory, transport_register,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::local_storage;
use crate::selection::construction::scalar_graph::structural::provenance;
use selected_instructions::{LocalStorageSlotId, SelectedLocalStorageSlot};
use semantic_vocabulary::IntegerValue;

pub(super) fn establish(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let (established, shape, stores) =
        crate::selection::zeroed_structural_input::storage(source, row)
            .ok_or_else(SelectedInstructionError::custody)?;
    if shape.byte_size == 0 {
        // The semantic result remains in the exact source frontier. No address,
        // physical home or access exists; retain construction before the next
        // executable instruction, including the block's terminator.
        builder.pending_provenance.operations.push(row.operation);
        builder
            .pending_provenance
            .fuel
            .extend(row.fuel.iter().cloned());
        return Ok(());
    }
    let slot = LocalStorageSlotId::Structural {
        operation: row.operation,
        place: established.place,
    };
    builder
        .transport
        .local_slots
        .push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: u32::from(shape.byte_size),
            alignment: shape.alignment,
        });
    // The address instruction charges construction once; the payload stores
    // carry the row's provenance without introducing additional semantic work.
    let pointer = local_storage::address(builder, row, slot, 0, u32::from(shape.byte_size), true)?;
    // Every scalar zero spelling is the all-zero bit pattern, so one
    // materialization serves every leaf width.
    let zero = transport_register(builder, established.place, 0)?;
    builder.emit(
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(0),
        },
        builder.constraints.keys.materialize_i64,
        &[zero],
        provenance(row),
    )?;
    for (offset, width) in stores {
        memory(
            builder,
            row,
            established.place,
            offset,
            u32::from(width),
            SelectedMemoryAccessRole::WritePlace,
        )?;
        builder.emit(
            SelectedInstructionKind::Store {
                byte_offset: offset,
                byte_size: width,
            },
            builder
                .constraints
                .keys
                .store
                .ok_or_else(SelectedInstructionError::custody)?,
            &[pointer, zero],
            provenance(row),
        )?;
    }
    // Publish the whole home to edge transport only after every leaf store.
    builder
        .transport
        .pointers
        .push((established.place, pointer));
    Ok(())
}
