//! Select byte observations from descriptor and derived value homes.
use super::*;

pub(in crate::selection) fn byte_observation(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    match row.kind {
        LegalizedScalarInstructionKind::ByteSequenceRead { .. } => byte_sequence_read(builder, row),
        LegalizedScalarInstructionKind::ByteSequenceLength {
            source,
            length_byte_offset,
        } => byte_sequence_length(builder, row, source, length_byte_offset),
        _ => Err(invalid()),
    }
}

fn byte_sequence_read(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let LegalizedScalarInstructionKind::ByteSequenceRead {
        source,
        index,
        length,
        obligation,
        accepted_fact,
    } = row.kind
    else {
        return Err(invalid());
    };
    let definition = row.result.ok_or_else(invalid)?;
    let (_, index_register, _, index_type) = builder.resolve(index).ok_or_else(invalid)?;
    if definition.scalar_type
        != ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| invalid())?)
        || index_type
            != ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?,
            )
    {
        return Err(invalid());
    }
    let pointer = backing_pointer(builder, row, source)?;
    let view = builder
        .transport
        .views
        .iter()
        .find(|view| view.place == source)
        .copied();
    let physical_index = if let Some(view) = view {
        let offset = transport_register(builder, source, 0)?;
        // The view chain proves O + L <= R <= U64::MAX. The checked read i < L
        // gives O + i < R, so only this integer offset sum is claimed exact.
        builder.emit(
            SelectedInstructionKind::ExactAddI64 {
                obligation,
                accepted_fact,
            },
            builder.constraints.keys.add_i64,
            &[view.byte_offset, index_register, offset],
            SelectedInstructionProvenance {
                operations: vec![row.operation],
                values: vec![index, length, view.root_length],
                obligations: vec![obligation],
                ..Default::default()
            },
        )?;
        offset
    } else {
        index_register
    };
    let output = builder.register(
        definition.value,
        definition.definition_site,
        definition.scalar_type,
    )?;
    memory(
        builder,
        row,
        source,
        0,
        1,
        SelectedMemoryAccessRole::ReadByteSequence {
            index,
            length,
            obligation,
            accepted_fact,
        },
    )?;
    builder.emit(
        SelectedInstructionKind::Load8Indexed,
        builder.constraints.keys.load8_indexed.ok_or_else(invalid)?,
        &[pointer, physical_index, output],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![index, length, definition.value],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(output)
}

pub(super) fn backing_pointer(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    source: PlaceId,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    if let Some(view) = builder
        .transport
        .views
        .iter()
        .find(|view| view.place == source)
    {
        return Ok(view.backing_pointer);
    }
    let descriptor = builder
        .transport
        .pointers
        .iter()
        .find(|(place, _)| *place == source)
        .map(|(_, register)| *register)
        .ok_or_else(invalid)?;
    let pointer = transport_register(builder, source, 0)?;
    memory(
        builder,
        row,
        source,
        0,
        8,
        SelectedMemoryAccessRole::ReadPlace,
    )?;
    builder.emit(
        SelectedInstructionKind::Load64 { byte_offset: 0 },
        builder.constraints.keys.load64.ok_or_else(invalid)?,
        &[descriptor, pointer],
        provenance(row),
    )?;
    Ok(pointer)
}

fn byte_sequence_length(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    source: PlaceId,
    length_byte_offset: u32,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let result = row.result.ok_or_else(invalid)?;
    if length_byte_offset != 8
        || result.scalar_type
            != ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?,
            )
    {
        return Err(invalid());
    }
    if let Some(view) = builder
        .transport
        .views
        .iter()
        .find(|view| view.place == source)
    {
        let input = view.byte_length;
        let output = builder.register(result.value, result.definition_site, result.scalar_type)?;
        builder.emit(
            SelectedInstructionKind::CopyI64,
            builder.constraints.keys.copy_i64,
            &[input, output],
            SelectedInstructionProvenance {
                operations: vec![row.operation],
                values: vec![result.value],
                fuel: row.fuel.clone(),
                ..Default::default()
            },
        )?;
        return Ok(output);
    }
    let input = builder
        .transport
        .pointers
        .iter()
        .find(|(place, _)| *place == source)
        .map(|(_, register)| *register)
        .ok_or_else(invalid)?;
    let output = builder.register(result.value, result.definition_site, result.scalar_type)?;
    memory(
        builder,
        row,
        source,
        length_byte_offset,
        8,
        SelectedMemoryAccessRole::ReadPlace,
    )?;
    let provenance = SelectedInstructionProvenance {
        operations: vec![row.operation],
        values: vec![result.value],
        fuel: row.fuel.clone(),
        ..Default::default()
    };
    builder.emit(
        SelectedInstructionKind::Load64 {
            byte_offset: length_byte_offset,
        },
        builder.constraints.keys.load64.ok_or_else(invalid)?,
        &[input, output],
        provenance,
    )?;
    Ok(output)
}
