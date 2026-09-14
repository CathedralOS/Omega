//! Independently replay the exact copy range and subsequent length publication.
//! Capacity never supplies readable source extent; unused destination bytes are
//! not observed or cleared. ISA-local scratch/control is not source control flow.
use super::*;

pub(super) fn replace(
    function: &LegalizedScalarFunction,
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::StructuralByteSequenceFieldStore {
        destination,
        field,
        source,
        length,
        obligation,
        accepted_fact,
    } = &row.kind
    else {
        return Err(replay.invalid());
    };
    let signature = function
        .structural
        .as_ref()
        .ok_or_else(|| replay.invalid())?;
    let parameter = signature
        .parameters
        .iter()
        .find(|parameter| parameter.semantic.place == destination.place)
        .ok_or_else(|| replay.invalid())?;
    if row.result.is_some()
        || parameter.semantic.access != destination.access
        || !matches!(
            destination.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        )
        || parameter.semantic.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || !parameter.semantic.qualifications.is_empty()
        || !parameter.semantic.projected_qualifications.is_empty()
        || *source == destination.place
        || crate::selection::established_view_input::view_type(function, *source).is_none()
    {
        return Err(replay.invalid());
    }
    let (metadata_offset, _) = crate::structural_reference_input::byte_field_storage(
        parameter.semantic.structural_type,
        &destination.path,
        *field,
        &signature.structural_types,
    )
    .ok_or_else(|| replay.invalid())?;
    let byte_offset = metadata_offset
        .checked_add(8)
        .ok_or_else(|| replay.invalid())?;
    let (_, count, _, count_type) = replay.resolve(*length).ok_or_else(|| replay.invalid())?;
    if count_type
        != ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| replay.invalid())?,
        )
    {
        return Err(replay.invalid());
    }
    let mut source_pointer = byte_views::backing_pointer(replay, row, *source)?;
    // A subslice retains root backing plus a view offset. Its own first byte,
    // rather than the backing's first byte, is the copy's source.
    if let Some(view) = replay
        .transport
        .views
        .iter()
        .find(|view| view.place == *source)
        .copied()
    {
        let address = result(replay, *source, 0)?;
        replay.check_instruction(
            SelectedInstructionKind::ByteViewAddress,
            replay.constraints.keys.add_i64,
            &[source_pointer, view.byte_offset, address],
            &provenance(row),
        )?;
        source_pointer = address;
    }
    let root = replay
        .transport
        .pointers
        .iter()
        .find(|(place, _)| *place == destination.place)
        .map(|(_, pointer)| *pointer)
        .ok_or_else(|| replay.invalid())?;
    let destination_pointer = result(replay, destination.place, byte_offset)?;
    replay.check_instruction(
        SelectedInstructionKind::AddressOffset { byte_offset },
        replay
            .constraints
            .keys
            .address_offset
            .ok_or_else(|| replay.invalid())?,
        &[root, destination_pointer],
        &provenance(row),
    )?;
    let cursor = scratch(replay, 3)?;
    let byte = scratch(replay, 4)?;
    memory(
        replay,
        row,
        *source,
        0,
        0,
        SelectedMemoryAccessRole::ReadByteSpan {
            length: *length,
            obligation: *obligation,
            accepted_fact: *accepted_fact,
        },
    )?;
    memory(
        replay,
        row,
        destination.place,
        byte_offset,
        0,
        SelectedMemoryAccessRole::WriteByteSpan {
            length: *length,
            obligation: *obligation,
            accepted_fact: *accepted_fact,
        },
    )?;
    replay.check_instruction(
        SelectedInstructionKind::CopyBytes,
        replay
            .constraints
            .keys
            .copy_bytes
            .ok_or_else(|| replay.invalid())?,
        &[source_pointer, destination_pointer, count, cursor, byte],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![*length],
            obligations: vec![*obligation],
            ..Default::default()
        },
    )?;
    memory(
        replay,
        row,
        destination.place,
        metadata_offset,
        8,
        SelectedMemoryAccessRole::WritePlace,
    )?;
    replay.check_instruction(
        SelectedInstructionKind::Store {
            byte_offset: metadata_offset,
            byte_size: 8,
        },
        replay
            .constraints
            .keys
            .store
            .ok_or_else(|| replay.invalid())?,
        &[root, count],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![*length],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )
}

fn scratch(
    replay: &mut Replay<'_>,
    operand: u16,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let instruction = SelectedInstructionId(
        replay
            .instruction_cursor
            .try_into()
            .map_err(|_| replay.invalid())?,
    );
    register(
        replay,
        VirtualRegisterOrigin::InstructionScratch {
            instruction,
            operand,
        },
        None,
    )
}
