//! Independently replay whole-field copy/length publication or one indexed byte.
//! Capacity never supplies readable source extent; unused destination bytes are
//! not observed or cleared. ISA-local scratch/control is not source control flow.
use super::{
    IntegerSign, IntegerType, LegalizedScalarFunction, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind, ScalarType, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedMemoryAccessRole, StructuralAccess, VirtualRegisterId,
    VirtualRegisterOrigin, memory,
};
use crate::SelectedInstructionError;
use crate::selection::validation::scalar_graph::Replay;
use crate::selection::validation::scalar_graph::structural::byte_views;
use crate::selection::validation::scalar_graph::structural::provenance;
use crate::selection::validation::scalar_graph::structural::register;
use crate::selection::validation::scalar_graph::structural::result;

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
    let (metadata_offset, _) =
        crate::structural_inputs::structural_reference_input::byte_field_storage(
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
    // The copy's source bytes live in the view's storage root, not the view's
    // own identity; a block-parameter view reaches each bound root.
    match crate::selection::byte_view_homes::view_backing_roots(
        function,
        &replay.transport.views,
        *source,
        *length,
    )? {
        Some(roots) => {
            for (root, extent) in roots {
                memory(
                    replay,
                    row,
                    root,
                    0,
                    0,
                    SelectedMemoryAccessRole::ReadByteSpan {
                        length: extent,
                        obligation: *obligation,
                        accepted_fact: *accepted_fact,
                    },
                )?;
            }
        }
        None => memory(
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
        )?,
    }
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

pub(super) fn replace_byte(
    function: &LegalizedScalarFunction,
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::StructuralByteSequenceFieldByteStore {
        destination,
        field,
        index,
        value,
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
    {
        return Err(replay.invalid());
    }
    let (metadata_offset, _) =
        crate::structural_inputs::structural_reference_input::byte_field_storage(
            parameter.semantic.structural_type,
            &destination.path,
            *field,
            &signature.structural_types,
        )
        .ok_or_else(|| replay.invalid())?;
    let payload_offset = metadata_offset
        .checked_add(8)
        .ok_or_else(|| replay.invalid())?;
    let (_, index_register, _, index_type) =
        replay.resolve(*index).ok_or_else(|| replay.invalid())?;
    let (_, value_register, _, value_type) =
        replay.resolve(*value).ok_or_else(|| replay.invalid())?;
    let (_, _, _, length_type) = replay.resolve(*length).ok_or_else(|| replay.invalid())?;
    let count_type = ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| replay.invalid())?,
    );
    if index_type != count_type
        || length_type != count_type
        || value_type
            != ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| replay.invalid())?,
            )
    {
        return Err(replay.invalid());
    }
    let root = replay
        .transport
        .pointers
        .iter()
        .find(|(place, _)| *place == destination.place)
        .map(|(_, pointer)| *pointer)
        .ok_or_else(|| replay.invalid())?;
    let payload = result(replay, destination.place, payload_offset)?;
    replay.check_instruction(
        SelectedInstructionKind::AddressOffset {
            byte_offset: payload_offset,
        },
        replay
            .constraints
            .keys
            .address_offset
            .ok_or_else(|| replay.invalid())?,
        &[root, payload],
        &provenance(row),
    )?;
    let address = result(replay, destination.place, payload_offset)?;
    replay.check_instruction(
        SelectedInstructionKind::ByteViewAddress,
        replay.constraints.keys.add_i64,
        &[payload, index_register, address],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![*index, *length],
            obligations: vec![*obligation],
            ..Default::default()
        },
    )?;
    memory(
        replay,
        row,
        destination.place,
        payload_offset,
        1,
        SelectedMemoryAccessRole::WriteByteSequence {
            index: *index,
            value: *value,
            length: *length,
            obligation: *obligation,
            accepted_fact: *accepted_fact,
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
            values: vec![*index, *value, *length],
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
