//! One byte of a bounded field's live storage. The byte is addressed in the
//! field's original inline backing, exactly where an indexed byte store
//! writes it; the live length is not loaded, since the index's bound is the
//! read's own accepted obligation against the field's current length
//! observation.
use super::{
    Builder, IntegerSign, IntegerType, LegalizedScalarFunction, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind, ScalarType, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedMemoryAccessRole, StructuralAccess, VirtualRegisterId,
    memory,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::invalid;
use crate::selection::construction::scalar_graph::structural::provenance;
use crate::selection::construction::scalar_graph::structural::transport_register;

pub(in crate::selection) fn read_byte(
    function: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
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
        return Err(invalid());
    };
    let definition = row.result.ok_or_else(invalid)?;
    let signature = function.structural.as_ref().ok_or_else(invalid)?;
    let parameter = signature
        .parameters
        .iter()
        .find(|parameter| parameter.semantic.place == source.place)
        .ok_or_else(invalid)?;
    if parameter.semantic.access != source.access
        || !matches!(
            source.access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        )
        || parameter.semantic.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || !parameter.semantic.qualifications.is_empty()
        || !parameter.semantic.projected_qualifications.is_empty()
    {
        return Err(invalid());
    }
    let (metadata_offset, _) =
        crate::structural_inputs::structural_reference_input::byte_field_storage(
            parameter.semantic.structural_type,
            &source.path,
            *field,
            &signature.structural_types,
        )
        .ok_or_else(invalid)?;
    let payload_offset = metadata_offset.checked_add(8).ok_or_else(invalid)?;
    let (_, index_register, _, index_type) = builder.resolve(*index).ok_or_else(invalid)?;
    let (_, _, _, length_type) = builder.resolve(*length).ok_or_else(invalid)?;
    let count_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?);
    if index_type != count_type
        || length_type != count_type
        || definition.scalar_type
            != ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| invalid())?,
            )
    {
        return Err(invalid());
    }
    let root = builder
        .transport
        .pointers
        .iter()
        .find(|(place, _)| *place == source.place)
        .map(|(_, pointer)| *pointer)
        .ok_or_else(invalid)?;
    let payload = transport_register(builder, source.place, payload_offset)?;
    builder.emit(
        SelectedInstructionKind::AddressOffset {
            byte_offset: payload_offset,
        },
        builder
            .constraints
            .keys
            .address_offset
            .ok_or_else(invalid)?,
        &[root, payload],
        provenance(row),
    )?;
    let output = builder.register(
        definition.value,
        definition.definition_site,
        definition.scalar_type,
    )?;
    memory(
        builder,
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
    builder.emit(
        SelectedInstructionKind::Load8Indexed,
        builder.constraints.keys.load8_indexed.ok_or_else(invalid)?,
        &[payload, index_register, output],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![*index, *length, definition.value],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(output)
}
