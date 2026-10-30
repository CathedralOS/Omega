//! Publish the structural result of one bounded compare-exchange instruction.
//! The CAS output is the instruction-observed resident. x86 CMPXCHG and ARM
//! LSE CAS complete without a retry loop, so these realizations select only
//! Mismatched or Exchanged. The third case remains in the outcome schema;
//! no contention or synthetic spurious failure is invented to select it.
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    ScalarType, SelectedInstructionKind, SelectedMemoryAccessRole, VirtualRegisterId, invalid,
    memory, provenance, transport_register,
};
use crate::SelectedInstructionError;

pub(super) fn establish(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
    observed: VirtualRegisterId,
    expected: VirtualRegisterId,
    scalar: ScalarType,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::AtomicAccess {
        outcome: Some((result, layout)),
        ..
    } = &row.kind
    else {
        return Err(invalid());
    };
    let ScalarType::Integer(integer) = scalar else {
        return Err(invalid());
    };
    let declaration = source
        .structural
        .as_ref()
        .and_then(|signature| {
            signature
                .structural_types
                .iter()
                .find(|row| row.id == result.structural_type)
        })
        .ok_or_else(invalid)?;
    if target_operations::conventional_atomic_once_layout(declaration, integer).as_ref()
        != Some(layout)
    {
        return Err(invalid());
    }
    let normalization = crate::selection::scalar_call_abi::integer_carrier_normalization(scalar);
    let prior = if normalization == SelectedInstructionKind::CopyI64 {
        observed
    } else {
        let normalized = transport_register(builder, result.place, 0)?;
        builder.emit(
            normalization,
            builder.constraints.keys.copy_i64,
            &[observed, normalized],
            provenance(row),
        )?;
        normalized
    };
    let tag = transport_register(builder, result.place, layout.tag_byte_offset)?;
    builder.emit(
        SelectedInstructionKind::CompareI64,
        builder.constraints.keys.compare_i64,
        &[prior, expected],
        provenance(row),
    )?;
    builder.emit(
        SelectedInstructionKind::MaterializeBooleanEqual,
        builder.constraints.keys.materialize_boolean,
        &[tag],
        provenance(row),
    )?;
    let pointer = super::zeroed_slot(builder, row, result.place, layout.shape)?;
    let payload = &layout.cases[0].fields[0];
    if layout.cases[2].fields != layout.cases[0].fields {
        return Err(invalid());
    }
    memory(
        builder,
        row,
        result.place,
        payload.byte_offset,
        payload.shape.byte_size,
        SelectedMemoryAccessRole::WritePlace,
    )?;
    builder.emit_store(
        payload.byte_offset,
        payload.shape.byte_size as u8,
        pointer,
        prior,
        provenance(row),
    )?;
    memory(
        builder,
        row,
        result.place,
        layout.tag_byte_offset,
        layout.tag_shape.byte_size,
        SelectedMemoryAccessRole::WritePlace,
    )?;
    builder.emit_store(
        layout.tag_byte_offset,
        layout.tag_shape.byte_size as u8,
        pointer,
        tag,
        provenance(row),
    )?;
    builder.transport.pointers.push((result.place, pointer));
    Ok(())
}
