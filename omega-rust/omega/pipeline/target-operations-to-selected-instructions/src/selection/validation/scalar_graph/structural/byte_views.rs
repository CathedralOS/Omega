//! Independently replay byte observations from descriptor and derived value homes.
use super::*;

// Store through the original backing pointer; the descriptor itself is unchanged.
pub(super) fn write(
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::ByteSequenceWrite {
        destination,
        index,
        value,
        length,
        obligation,
        accepted_fact,
    } = row.kind
    else {
        return Err(replay.invalid());
    };
    let (_, index_register, _, index_type) =
        replay.resolve(index).ok_or_else(|| replay.invalid())?;
    let (_, value_register, _, value_type) =
        replay.resolve(value).ok_or_else(|| replay.invalid())?;
    if row.result.is_some()
        || index_type
            != ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| replay.invalid())?,
            )
        || value_type
            != ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| replay.invalid())?,
            )
        || replay
            .transport
            .views
            .iter()
            .any(|view| view.place == destination)
    {
        return Err(replay.invalid());
    }
    let pointer = backing_pointer(replay, row, destination)?;
    let address = result(replay, destination, 0)?;
    replay.check_instruction(
        SelectedInstructionKind::ByteViewAddress,
        replay.constraints.keys.add_i64,
        &[pointer, index_register, address],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![index, length],
            obligations: vec![obligation],
            ..Default::default()
        },
    )?;
    memory(
        replay,
        row,
        destination,
        0,
        1,
        SelectedMemoryAccessRole::WriteByteSequence {
            index,
            value,
            length,
            obligation,
            accepted_fact,
        },
    )?;
    replay.check_instruction(
        SelectedInstructionKind::Store {
            byte_offset: 0,
            byte_size: 1,
        },
        replay
            .constraints
            .keys
            .store
            .ok_or_else(|| replay.invalid())?,
        &[address, value_register],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![index, value, length],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )
}

pub(in crate::selection) fn byte_observation(
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    match row.kind {
        LegalizedScalarInstructionKind::ByteSequenceRead { .. } => byte_sequence_read(replay, row),
        LegalizedScalarInstructionKind::ByteSequenceLength {
            source,
            length_byte_offset,
        } => byte_sequence_length(replay, row, source, length_byte_offset),
        _ => Err(replay.invalid()),
    }
}

fn byte_sequence_read(
    replay: &mut Replay<'_>,
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
        return Err(replay.invalid());
    };
    let definition = row.result.ok_or_else(|| replay.invalid())?;
    let (_, index_register, _, index_type) =
        replay.resolve(index).ok_or_else(|| replay.invalid())?;
    if definition.scalar_type
        != ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| replay.invalid())?,
        )
        || index_type
            != ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| replay.invalid())?,
            )
    {
        return Err(replay.invalid());
    }
    let pointer = backing_pointer(replay, row, source)?;
    let view = replay
        .transport
        .views
        .iter()
        .find(|view| view.place == source)
        .copied();
    let physical_index = match view {
        Some(view) => {
            let offset = result(replay, source, 0)?;
            // The replayed view chain gives O + L <= R <= U64::MAX; this exact
            // view's read fact gives i < L. Thus O + i < R is an exact U64 sum.
            replay.check_instruction(
                SelectedInstructionKind::ExactAddI64 {
                    obligation,
                    accepted_fact,
                },
                replay.constraints.keys.add_i64,
                &[view.byte_offset, index_register, offset],
                &SelectedInstructionProvenance {
                    operations: vec![row.operation],
                    values: vec![index, length, view.root_length],
                    obligations: vec![obligation],
                    ..Default::default()
                },
            )?;
            offset
        }
        None => index_register,
    };
    let output = replay.result_register(
        definition.value,
        definition.definition_site,
        definition.scalar_type,
    )?;
    memory(
        replay,
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
    replay.check_instruction(
        SelectedInstructionKind::Load8Indexed,
        replay
            .constraints
            .keys
            .load8_indexed
            .ok_or_else(|| replay.invalid())?,
        &[pointer, physical_index, output],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![index, length, definition.value],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(output)
}

pub(super) fn backing_pointer(
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
    source: PlaceId,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    if let Some(view) = replay
        .transport
        .views
        .iter()
        .find(|view| view.place == source)
    {
        return Ok(view.backing_pointer);
    }
    let descriptor = replay
        .transport
        .pointers
        .iter()
        .find(|(place, _)| *place == source)
        .map(|(_, register)| *register)
        .ok_or_else(|| replay.invalid())?;
    let pointer = result(replay, source, 0)?;
    memory(
        replay,
        row,
        source,
        0,
        8,
        SelectedMemoryAccessRole::ReadPlace,
    )?;
    replay.check_instruction(
        SelectedInstructionKind::Load64 { byte_offset: 0 },
        replay
            .constraints
            .keys
            .load64
            .ok_or_else(|| replay.invalid())?,
        &[descriptor, pointer],
        &provenance(row),
    )?;
    Ok(pointer)
}

fn byte_sequence_length(
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
    source: PlaceId,
    length_byte_offset: u32,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let result = row.result.ok_or_else(|| replay.invalid())?;
    if length_byte_offset != 8
        || result.scalar_type
            != ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| replay.invalid())?,
            )
    {
        return Err(replay.invalid());
    }
    if let Some(view) = replay
        .transport
        .views
        .iter()
        .find(|view| view.place == source)
    {
        let input = view.byte_length;
        let output =
            replay.result_register(result.value, result.definition_site, result.scalar_type)?;
        replay.check_instruction(
            SelectedInstructionKind::CopyI64,
            replay.constraints.keys.copy_i64,
            &[input, output],
            &SelectedInstructionProvenance {
                operations: vec![row.operation],
                values: vec![result.value],
                fuel: row.fuel.clone(),
                ..Default::default()
            },
        )?;
        return Ok(output);
    }
    let input = replay
        .transport
        .pointers
        .iter()
        .find(|(place, _)| *place == source)
        .map(|(_, register)| *register)
        .ok_or_else(|| replay.invalid())?;
    let output =
        replay.result_register(result.value, result.definition_site, result.scalar_type)?;
    memory(
        replay,
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
    replay.check_instruction(
        SelectedInstructionKind::Load64 {
            byte_offset: length_byte_offset,
        },
        replay
            .constraints
            .keys
            .load64
            .ok_or_else(|| replay.invalid())?,
        &[input, output],
        &provenance,
    )?;
    Ok(output)
}
