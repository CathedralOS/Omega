//! Independently replay one indexed byte read from a bounded field's inline
//! backing: the field's payload address, the read's memory row under its own
//! accepted obligation, and the byte load. The live length is never loaded.
use super::{
    IntegerSign, IntegerType, LegalizedScalarFunction, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind, ScalarType, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedMemoryAccessRole, StructuralAccess, VirtualRegisterId,
    memory,
};
use crate::SelectedInstructionError;
use crate::selection::validation::scalar_graph::Replay;
use crate::selection::validation::scalar_graph::structural::provenance;
use crate::selection::validation::scalar_graph::structural::result;

pub(in crate::selection) fn read_byte(
    function: &LegalizedScalarFunction,
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let LegalizedScalarInstructionKind::StructuralByteSequenceFieldRead {
        source,
        field,
        index,
        length,
        obligation,
        accepted_fact,
    } = &row.kind
    else {
        return Err(replay.invalid());
    };
    let definition = row.result.ok_or_else(|| replay.invalid())?;
    let signature = function
        .structural
        .as_ref()
        .ok_or_else(|| replay.invalid())?;
    let parameter = signature
        .parameters
        .iter()
        .find(|parameter| parameter.semantic.place == source.place)
        .ok_or_else(|| replay.invalid())?;
    if parameter.semantic.access != source.access
        || !matches!(
            source.access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
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
            &source.path,
            *field,
            &signature.structural_types,
        )
        .ok_or_else(|| replay.invalid())?;
    let payload_offset = metadata_offset
        .checked_add(8)
        .ok_or_else(|| replay.invalid())?;
    let (_, index_register, _, index_type) =
        replay.resolve(*index).ok_or_else(|| replay.invalid())?;
    let (_, _, _, length_type) = replay.resolve(*length).ok_or_else(|| replay.invalid())?;
    let count_type = ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| replay.invalid())?,
    );
    if index_type != count_type
        || length_type != count_type
        || definition.scalar_type
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
        .find(|(place, _)| *place == source.place)
        .map(|(_, pointer)| *pointer)
        .ok_or_else(|| replay.invalid())?;
    let payload = result(replay, source.place, payload_offset)?;
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
    let output = replay.result_register(
        definition.value,
        definition.definition_site,
        definition.scalar_type,
    )?;
    memory(
        replay,
        row,
        source.place,
        payload_offset,
        1,
        SelectedMemoryAccessRole::ReadByteSequence {
            index: *index,
            length: *length,
            obligation: *obligation,
            accepted_fact: *accepted_fact,
        },
    )?;
    replay.check_instruction(
        SelectedInstructionKind::Load8Indexed,
        replay
            .constraints
            .keys
            .load8_indexed
            .ok_or_else(|| replay.invalid())?,
        &[payload, index_register, output],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![*index, *length, definition.value],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(output)
}
