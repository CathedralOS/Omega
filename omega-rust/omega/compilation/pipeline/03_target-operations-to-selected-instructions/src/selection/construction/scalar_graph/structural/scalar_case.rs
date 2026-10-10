//! Initialize a complete sum carrier before publishing its tag and owned payloads.
use super::super::{IntegerValue, ValueId};
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    SelectedInstructionKind, SelectedInstructionProvenance, SelectedMemoryAccessRole,
    VirtualRegisterId, memory,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::invalid;
use crate::selection::construction::scalar_graph::structural::local_storage;
use crate::selection::construction::scalar_graph::structural::provenance;
use crate::selection::construction::scalar_graph::structural::transport_register;

pub(super) fn establish(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let (ordinal, placements) =
        crate::selection::aggregate_result_input::fields(source, row).ok_or_else(|| invalid())?;
    let LegalizedScalarInstructionKind::EstablishCase {
        result,
        fields,
        layout,
        ..
    } = &row.kind
    else {
        return Err(invalid());
    };
    let slot = builder.transport.push_structural_local_slot(
        row.operation,
        result.place,
        layout.shape.byte_size,
        layout.shape.alignment,
    );
    let pointer = local_storage::address(builder, row, slot, 0, layout.shape.byte_size, true)?;
    let zero = transport_register(builder, result.place, 0)?;
    builder.emit(
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(0),
        },
        builder.constraints.keys.materialize_i64,
        &[zero],
        provenance(row),
    )?;
    let mut offset = 0;
    while offset < layout.shape.byte_size {
        let remaining = layout.shape.byte_size - offset;
        let width = [8, 4, 2, 1]
            .into_iter()
            .find(|width| *width <= remaining)
            .ok_or_else(|| invalid())?;
        store(builder, row, pointer, offset, width as u8, zero, Vec::new())?;
        offset += width;
    }
    let tag = transport_register(builder, result.place, 0)?;
    builder.emit(
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(ordinal as u128),
        },
        builder.constraints.keys.materialize_i64,
        &[tag],
        provenance(row),
    )?;
    store(builder, row, pointer, 0, 4, tag, Vec::new())?;
    super::record::write_fields(
        source,
        row,
        builder,
        result.place,
        result.structural_type,
        pointer,
        fields,
        &placements,
    )?;
    // Publish to ordinary edge transport only after the carrier is initialized.
    builder.transport.pointers.push((result.place, pointer));
    Ok(())
}

fn store(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    pointer: VirtualRegisterId,
    offset: u32,
    width: u8,
    value: VirtualRegisterId,
    values: Vec<ValueId>,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::EstablishCase { result, .. } = &row.kind else {
        return Err(invalid());
    };
    memory(
        builder,
        row,
        result.place,
        offset,
        u32::from(width),
        SelectedMemoryAccessRole::WritePlace,
    )?;
    builder.emit_store(
        offset,
        width,
        pointer,
        value,
        SelectedInstructionProvenance {
            values,
            ..provenance(row)
        },
    )
}
