//! Establish a memberwise structural local's home: the slot begins stale
//! (zeroed for a defined starting byte pattern) and the member stores that
//! complete it arrive as their own operations.
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, SelectedInstructionKind,
    transport_register,
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
    let (established, shape) = crate::selection::structural_local_input::storage(source, row)
        .ok_or_else(SelectedInstructionError::custody)?;
    let slot = LocalStorageSlotId::Structural {
        operation: row.operation,
        place: established.place,
    };
    builder
        .transport
        .local_slots
        .push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: shape.byte_size.max(1),
            alignment: shape.alignment,
        });
    let pointer = local_storage::address(builder, row, slot, 0, shape.byte_size.max(1), true)?;
    if shape.byte_size == 0 {
        builder
            .transport
            .pointers
            .push((established.place, pointer));
        return Ok(());
    }
    // The member stores overwrite the members they establish; the remaining
    // bytes stay zero like a record home's padding, so a whole-value copy
    // transports no indeterminate byte.
    let zero = transport_register(builder, established.place, 0)?;
    builder.emit(
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(0),
        },
        builder.constraints.keys.materialize_i64,
        &[zero],
        provenance(row),
    )?;
    let mut offset = 0u32;
    while offset < shape.byte_size {
        let width = super::record::chunk(shape.byte_size - offset, offset, offset);
        super::record::store(
            builder,
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
    // Publish the whole home to edge transport only after every store.
    builder
        .transport
        .pointers
        .push((established.place, pointer));
    Ok(())
}
