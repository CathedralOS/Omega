//! Address a byte field's original backing and select an indexed observation.
//! Inline owners and borrowed descriptors use the same payload-address owner
//! as measured sequence operations. The index's own accepted obligation binds
//! its current length; finding an address grants no permission to read it.
use super::{
    Builder, IntegerSign, IntegerType, LegalizedScalarFunction, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind, ScalarType, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedMemoryAccessRole, VirtualRegisterId, memory,
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
        indices,
    } = &row.kind
    else {
        return Err(invalid());
    };
    let definition = row.result.ok_or_else(invalid)?;
    let geometry =
        crate::selection::field_bytes::FieldBytes::resolve(function, source, *field, indices)
            .ok_or_else(|| invalid())?;
    let payload_offset = geometry.offset();
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
    let payload = payload_pointer(function, builder, row, source.place, geometry, indices)?;
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

/// Address the field's actual byte backing for either an indexed observation
/// or a measured sequence operation. Loading a descriptor is a separate
/// recorded read; observing its backing retains the caller's own extent proof.
pub(super) fn payload_pointer(
    function: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: semantic_vocabulary::PlaceId,
    geometry: crate::selection::field_bytes::FieldBytes,
    indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let offset = geometry.offset();
    let root = super::runtime_address::projection_base(function, builder, row, place, offset)?
        .ok_or_else(|| invalid())?;
    let root = super::runtime_address::scale(builder, row, place, offset, root, indices)?;
    let payload = transport_register(builder, place, offset)?;
    match geometry {
        crate::selection::field_bytes::FieldBytes::Inline(_) => builder.emit(
            SelectedInstructionKind::AddressOffset {
                byte_offset: offset,
            },
            builder
                .constraints
                .keys
                .address_offset
                .ok_or_else(|| invalid())?,
            &[root, payload],
            provenance(row),
        )?,
        crate::selection::field_bytes::FieldBytes::Descriptor(_) => {
            super::runtime_address::footprint(
                builder,
                row,
                place,
                offset,
                8,
                indices,
                super::runtime_address::Written::None,
            )?;
            builder.emit(
                SelectedInstructionKind::Load64 {
                    byte_offset: offset,
                },
                builder.constraints.keys.load64.ok_or_else(|| invalid())?,
                &[root, payload],
                provenance(row),
            )?;
        }
    }
    Ok(payload)
}
