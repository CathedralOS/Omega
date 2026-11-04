//! Store the observed resident and outcome produced by one atomic attempt.
//! The native form owns the commit decision. Equality alone cannot distinguish
//! an exchanged value from a matching load whose exclusive store failed.
//! The checked canonical layout still owns the tag and payload storage.
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
    tag: VirtualRegisterId,
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
    let pointer = super::record::initialize_storage(row, builder, result.place, layout.shape)?;
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
