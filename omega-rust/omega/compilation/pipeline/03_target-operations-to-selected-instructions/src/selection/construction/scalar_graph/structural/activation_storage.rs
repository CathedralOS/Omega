//! Caller-owned bounded backing stays at its establishment site. Only the
//! ordinary sum/claim descriptor prefix participates in owned edge transport.
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    SelectedInstructionKind, SelectedMemoryAccessRole, memory, provenance, transport_register,
};
use crate::SelectedInstructionError;
use semantic_vocabulary::IntegerValue;

pub(super) fn emit(
    source: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<(), SelectedInstructionError> {
    let invalid = SelectedInstructionError::custody;
    if matches!(
        row.kind,
        LegalizedScalarInstructionKind::SplitActivationClaim { .. }
            | LegalizedScalarInstructionKind::MergeActivationClaim { .. }
    ) {
        return emit_division(builder, row);
    }
    if let LegalizedScalarInstructionKind::ReleaseActivationClaim { source, .. } = &row.kind {
        // Retirement is statically proved; backing remains reserved until frame
        // unwind. Retain an executable fuel/provenance anchor for the operation.
        let anchor = transport_register(builder, source.place, 0)?;
        let mut origin = provenance(row);
        origin.fuel = row.fuel.clone();
        return builder.emit_materialize_i64(IntegerValue::Unsigned(0), anchor, origin);
    }
    let LegalizedScalarInstructionKind::EstablishActivationClaim {
        result,
        bound,
        committed,
        established_case,
        rejected_case,
        layout,
        ..
    } = &row.kind
    else {
        return Err(invalid());
    };
    let declarations = &source
        .structural
        .as_ref()
        .ok_or_else(invalid)?
        .structural_types;
    let declaration = declarations
        .iter()
        .find(|ty| ty.id == result.structural_type)
        .ok_or_else(invalid)?;
    let terminal_psi::StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return Err(invalid());
    };
    if terminal_psi::structural_case_position(cases, *established_case) != Some(0)
        || terminal_psi::structural_case_position(cases, *rejected_case) != Some(1)
        || cases.len() != 2
        || cases[0].fields.len() != 1
        || !cases[1].fields.is_empty()
        || layout.cases.len() != 2
        || layout.cases[0].fields.len() != 1
    {
        return Err(invalid());
    }
    let bound_bytes = u32::try_from(*bound).map_err(|_| invalid())?;
    let backing_offset = layout
        .shape
        .byte_size
        .checked_add(7)
        .map(|size| size & !7)
        .ok_or_else(invalid)?;
    let payload_offset = backing_offset.checked_add(8).ok_or_else(invalid)?;
    let storage_shape = crate::selection::activation_storage_input::footprint(layout.shape, *bound)
        .ok_or_else(invalid)?;
    let total = payload_offset
        .checked_add(bound_bytes)
        .ok_or_else(invalid)?;
    if total != storage_shape.byte_size {
        return Err(invalid());
    }
    let slot = builder.transport.push_structural_local_slot(
        row.operation,
        result.place,
        total,
        storage_shape.alignment,
    );
    let pointer = super::local_storage::address(builder, row, slot, 0, total, true)?;
    let count = transport_register(builder, result.place, 0)?;
    builder.emit_materialize_i64(
        IntegerValue::Unsigned(u128::from(total)),
        count,
        provenance(row),
    )?;
    let cursor = super::byte_field_store::scratch(builder, 2)?;
    memory(
        builder,
        row,
        result.place,
        0,
        total,
        SelectedMemoryAccessRole::WriteLocal { slot },
    )?;
    builder.emit(
        SelectedInstructionKind::ZeroBytes,
        builder.constraints.keys.zero_bytes.ok_or_else(invalid)?,
        &[pointer, count, cursor],
        provenance(row),
    )?;
    let (_, committed_register, _, committed_type) =
        builder.resolve(*committed).ok_or_else(invalid)?;
    if !matches!(committed_type, super::ScalarType::Integer(integer) if integer.bits() == 64 && integer.sign() == super::IntegerSign::Unsigned && integer.carrier() == semantic_vocabulary::IntegerCarrier::Fixed)
    {
        return Err(invalid());
    }
    let bound_register = transport_register(builder, result.place, 0)?;
    builder.emit_materialize_i64(
        IntegerValue::Unsigned(u128::from(*bound)),
        bound_register,
        provenance(row),
    )?;
    let tag = transport_register(builder, result.place, 0)?;
    builder.emit(
        SelectedInstructionKind::CompareI64,
        builder.constraints.keys.compare_i64,
        &[bound_register, committed_register],
        provenance(row),
    )?;
    builder.emit(
        SelectedInstructionKind::MaterializeBooleanU64LessThan,
        builder.constraints.keys.materialize_boolean,
        &[tag],
        provenance(row),
    )?;
    memory(
        builder,
        row,
        result.place,
        layout.tag_byte_offset,
        4,
        SelectedMemoryAccessRole::WriteLocal { slot },
    )?;
    builder.emit_store(layout.tag_byte_offset, 4, pointer, tag, provenance(row))?;
    let data = transport_register(builder, result.place, payload_offset)?;
    builder.emit_address_offset(payload_offset, pointer, data, provenance(row))?;
    let descriptor_offset = layout.cases[0].fields[0].byte_offset;
    super::local_storage::store(builder, row, slot, descriptor_offset, data)?;
    super::local_storage::store(
        builder,
        row,
        slot,
        descriptor_offset.checked_add(8).ok_or_else(invalid)?,
        committed_register,
    )?;
    super::local_storage::store(builder, row, slot, backing_offset, committed_register)?;
    builder.transport.pointers.push((result.place, pointer));
    Ok(())
}

/// Split and merge copy claim descriptors. They do not reserve another
/// backing slot: both children and the merged claim keep the parent's pointer.
fn emit_division(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<(), SelectedInstructionError> {
    let invalid = SelectedInstructionError::custody;
    let (result, source, layout) = match &row.kind {
        LegalizedScalarInstructionKind::SplitActivationClaim {
            result,
            source,
            layout,
            ..
        } => (result, source, layout),
        LegalizedScalarInstructionKind::MergeActivationClaim {
            result,
            lower,
            layout,
            ..
        } => (result, lower, layout),
        _ => return Err(invalid()),
    };
    if layout.cases.len() != 2 || layout.cases[0].fields.is_empty() {
        return Err(invalid());
    }
    let total = layout.shape.byte_size;
    let slot = builder.transport.push_structural_local_slot(
        row.operation,
        result.place,
        total,
        layout.shape.alignment,
    );
    let pointer = super::local_storage::address(builder, row, slot, 0, total, true)?;
    let count = transport_register(builder, result.place, 0)?;
    let cursor = transport_register(builder, result.place, 1)?;
    builder.emit_materialize_i64(
        IntegerValue::Unsigned(u128::from(total)),
        count,
        provenance(row),
    )?;
    memory(
        builder,
        row,
        result.place,
        0,
        total,
        SelectedMemoryAccessRole::WriteLocal { slot },
    )?;
    let zero_bytes = builder.constraints.keys.zero_bytes.ok_or_else(invalid)?;
    builder.emit(
        SelectedInstructionKind::ZeroBytes,
        zero_bytes,
        &[pointer, count, cursor],
        provenance(row),
    )?;
    let tag = transport_register(builder, result.place, 0)?;
    builder.emit_materialize_i64(IntegerValue::Unsigned(0), tag, provenance(row))?;
    memory(
        builder,
        row,
        result.place,
        layout.tag_byte_offset,
        4,
        SelectedMemoryAccessRole::WriteLocal { slot },
    )?;
    builder.emit_store(layout.tag_byte_offset, 4, pointer, tag, provenance(row))?;
    let claim = super::place_pointer(builder, row, source.place, 0)?;
    let parent_pointer = transport_register(builder, result.place, 2)?;
    let parent_length = transport_register(builder, result.place, 3)?;
    builder.emit_load64(0, claim, parent_pointer, provenance(row))?;
    builder.emit_load64(8, claim, parent_length, provenance(row))?;
    let fields = if matches!(
        row.kind,
        LegalizedScalarInstructionKind::SplitActivationClaim { .. }
    ) {
        &layout.cases[0].fields
    } else {
        &layout.cases[0].fields[..1]
    };
    for field in fields {
        super::local_storage::store(builder, row, slot, field.byte_offset, parent_pointer)?;
        super::local_storage::store(
            builder,
            row,
            slot,
            field.byte_offset.checked_add(8).ok_or_else(invalid)?,
            parent_length,
        )?;
    }
    builder.transport.pointers.push((result.place, pointer));
    Ok(())
}
