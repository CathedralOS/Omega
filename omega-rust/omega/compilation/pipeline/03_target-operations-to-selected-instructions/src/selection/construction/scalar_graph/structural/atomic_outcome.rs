//! Store the payload and outcome produced by one atomic attempt.
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
        event,
        ..
    } = &row.kind
    else {
        return Err(invalid());
    };
    let custody = event.single_attempt_custody().ok_or_else(invalid)?;
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
    if target_operations::conventional_atomic_once_layout(declaration, integer, custody).as_ref()
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
    let value = match custody.observation {
        language_core::atomic::AtomicCompareExchangeObservationAxis::Observing => prior,
        language_core::atomic::AtomicCompareExchangeObservationAxis::NonObserving => {
            let terminal_psi::AtomicAccessEvent::CompareExchangeOnce { replacement, .. } = event
            else {
                return Err(invalid());
            };
            let (_, proposed, _, proposed_type) =
                builder.resolve(*replacement).ok_or_else(invalid)?;
            if proposed_type != scalar {
                return Err(invalid());
            }
            // Only the native tag establishes commitment. A matching expected
            // value can still produce Uncommitted, which must retain proposal.
            let one = super::fragmented_scalar::constant(builder, row, result.place, 1)?;
            let zero = super::fragmented_scalar::constant(builder, row, result.place, 0)?;
            builder.emit(
                SelectedInstructionKind::CompareI64,
                builder.constraints.keys.compare_i64,
                &[tag, one],
                provenance(row),
            )?;
            let committed = transport_register(builder, result.place, 0)?;
            builder.emit(
                SelectedInstructionKind::MaterializeBooleanEqual,
                builder.constraints.keys.materialize_boolean,
                &[committed],
                provenance(row),
            )?;
            let mask = super::fragmented_scalar::binary(
                builder,
                row,
                result.place,
                SelectedInstructionKind::WrappingSubtractI64,
                zero,
                committed,
            )?;
            let difference = super::fragmented_scalar::binary(
                builder,
                row,
                result.place,
                SelectedInstructionKind::BitwiseXorI64,
                prior,
                proposed,
            )?;
            let selected = super::fragmented_scalar::binary(
                builder,
                row,
                result.place,
                SelectedInstructionKind::BitwiseAndI64,
                difference,
                mask,
            )?;
            super::fragmented_scalar::binary(
                builder,
                row,
                result.place,
                SelectedInstructionKind::BitwiseXorI64,
                proposed,
                selected,
            )?
        }
    };
    let pointer = super::record::initialize_storage(row, builder, result.place, layout.shape)?;
    let payload = &layout.cases[0].fields[0];
    if layout.cases[2].fields != layout.cases[0].fields
        || (custody.observation
            == language_core::atomic::AtomicCompareExchangeObservationAxis::NonObserving
            && layout.cases[1].fields != layout.cases[0].fields)
    {
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
        value,
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
