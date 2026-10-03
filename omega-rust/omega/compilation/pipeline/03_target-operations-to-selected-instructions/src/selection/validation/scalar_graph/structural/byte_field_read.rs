//! Independently replay byte-field addressing and indexed observation. Inline
//! payload and descriptor-backed payload have distinct address instructions;
//! measured sequence operations reuse this same reconstruction. The read's
//! own accepted obligation supplies the extent, not the address calculation.
use super::{
    IntegerSign, IntegerType, LegalizedScalarFunction, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind, ScalarType, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedMemoryAccessRole, VirtualRegisterId, memory,
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
        indices,
    } = &row.kind
    else {
        return Err(replay.invalid());
    };
    let definition = row.result.ok_or_else(|| replay.invalid())?;
    let geometry =
        crate::selection::field_bytes::FieldBytes::resolve(function, source, *field, indices)
            .ok_or_else(|| replay.invalid())?;
    let payload_offset = geometry.offset();
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
    let payload = payload_pointer(function, replay, row, source.place, geometry, indices)?;
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

/// Address the field's actual byte backing for either an indexed observation
/// or a measured sequence operation. Loading a descriptor is a separate
/// recorded read; observing its backing retains the caller's own extent proof.
pub(super) fn payload_pointer(
    function: &LegalizedScalarFunction,
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
    place: semantic_vocabulary::PlaceId,
    geometry: crate::selection::field_bytes::FieldBytes,
    indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let offset = geometry.offset();
    let root = super::runtime_address::projection_base(function, replay, row, place, offset)?
        .ok_or_else(|| replay.invalid())?;
    let root = super::runtime_address::scale(replay, row, place, offset, root, indices)?;
    let payload = result(replay, place, offset)?;
    match geometry {
        crate::selection::field_bytes::FieldBytes::Inline(_) => replay.check_instruction(
            SelectedInstructionKind::AddressOffset {
                byte_offset: offset,
            },
            replay
                .constraints
                .keys
                .address_offset
                .ok_or_else(|| replay.invalid())?,
            &[root, payload],
            &provenance(row),
        )?,
        crate::selection::field_bytes::FieldBytes::Descriptor(_) => {
            super::runtime_address::footprint(
                replay,
                row,
                place,
                offset,
                8,
                indices,
                super::runtime_address::Written::None,
            )?;
            replay.check_instruction(
                SelectedInstructionKind::Load64 {
                    byte_offset: offset,
                },
                replay
                    .constraints
                    .keys
                    .load64
                    .ok_or_else(|| replay.invalid())?,
                &[root, payload],
                &provenance(row),
            )?;
        }
    }
    Ok(payload)
}
