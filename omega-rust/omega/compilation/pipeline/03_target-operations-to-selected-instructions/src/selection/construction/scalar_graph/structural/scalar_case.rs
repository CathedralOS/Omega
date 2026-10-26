//! Initialize a complete sum carrier before publishing its tag and owned payloads.
use super::super::{IntegerValue, ValueId};
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    SelectedInstructionProvenance, SelectedMemoryAccessRole, VirtualRegisterId, memory,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::invalid;
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
    let pointer = super::zeroed_slot(builder, row, result.place, layout.shape)?;
    let tag = transport_register(builder, result.place, 0)?;
    builder.emit_materialize_i64(
        IntegerValue::Unsigned(ordinal as u128),
        tag,
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
