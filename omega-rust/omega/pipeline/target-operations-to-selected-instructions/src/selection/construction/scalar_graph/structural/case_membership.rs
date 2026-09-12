//! Observe a sum tag without disturbing its payload or ownership.
use super::*;

pub(in crate::selection) fn observe(
    source: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let LegalizedScalarInstructionKind::StructuralCaseMembership {
        source: place,
        case,
        case_tag,
    } = row.kind
    else {
        return Err(invalid());
    };
    let result = row.result.ok_or_else(invalid)?;
    if result.scalar_type != ScalarType::Boolean
        || crate::selection::aggregate_result_input::membership_tag(source, place, case)
            != Some(case_tag)
    {
        return Err(invalid());
    }
    let tag = transport_register(builder, place, 0)?;
    let provenance = SelectedInstructionProvenance {
        operations: vec![row.operation],
        values: vec![result.value],
        fuel: row.fuel.clone(),
        ..Default::default()
    };
    if let Some(pointer) = builder
        .transport
        .pointers
        .iter()
        .find(|(stored, _)| *stored == place)
        .map(|(_, pointer)| *pointer)
    {
        memory(
            builder,
            row,
            place,
            0,
            4,
            SelectedMemoryAccessRole::ReadPlace,
        )?;
        builder.emit(
            SelectedInstructionKind::Load32 { byte_offset: 0 },
            builder.constraints.keys.load32.ok_or_else(invalid)?,
            &[pointer, tag],
            provenance,
        )?;
    } else {
        // Owned ABI fragments contain value bytes. Narrow the tag before
        // comparing so adjacent payload bits cannot affect membership.
        let fragment = builder
            .transport
            .fragments
            .iter()
            .find(|(stored, offset, _)| *stored == place && *offset == 0)
            .map(|(_, _, register)| *register)
            .ok_or_else(invalid)?;
        builder.emit(
            SelectedInstructionKind::ZeroExtendU32,
            builder.constraints.keys.copy_i64,
            &[fragment, tag],
            provenance,
        )?;
    }
    let expected = transport_register(builder, place, 0)?;
    builder.emit(
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(u128::from(case_tag)),
        },
        builder.constraints.keys.materialize_i64,
        &[expected],
        Default::default(),
    )?;
    builder.emit(
        SelectedInstructionKind::CompareI64,
        builder.constraints.keys.compare_i64,
        &[tag, expected],
        Default::default(),
    )?;
    let output = builder.register(result.value, result.definition_site, ScalarType::Boolean)?;
    builder.emit(
        SelectedInstructionKind::MaterializeBooleanEqual,
        builder.constraints.keys.materialize_boolean,
        &[output],
        SelectedInstructionProvenance {
            values: vec![result.value],
            ..Default::default()
        },
    )?;
    Ok(output)
}
