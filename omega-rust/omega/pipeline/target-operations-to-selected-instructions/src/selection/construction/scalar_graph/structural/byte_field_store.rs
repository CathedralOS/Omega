//! Whole replacement copies live bytes before publishing the new length;
//! indexed replacement addresses one byte in that same original inline backing.
//! Capacity never supplies readable source extent; unused destination bytes are
//! not observed or cleared. ISA-local scratch/control is not source control flow.
use super::{
    Builder, IntegerSign, IntegerType, LegalizedScalarFunction, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind, ScalarType, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedMemoryAccessRole, StructuralAccess, VirtualRegister,
    VirtualRegisterId, VirtualRegisterOrigin, memory,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::byte_views;
use crate::selection::construction::scalar_graph::structural::invalid;
use crate::selection::construction::scalar_graph::structural::provenance;
use crate::selection::construction::scalar_graph::structural::transport_register;

pub(super) fn replace(
    function: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
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
        return Err(invalid());
    };
    let signature = function.structural.as_ref().ok_or_else(invalid)?;
    let parameter = signature
        .parameters
        .iter()
        .find(|parameter| parameter.semantic.place == destination.place)
        .ok_or_else(invalid)?;
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
        return Err(invalid());
    }
    let (metadata_offset, _) =
        crate::structural_inputs::structural_reference_input::byte_field_storage(
            parameter.semantic.structural_type,
            &destination.path,
            *field,
            &signature.structural_types,
        )
        .ok_or_else(invalid)?;
    let byte_offset = metadata_offset.checked_add(8).ok_or_else(invalid)?;
    let (_, count, _, count_type) = builder.resolve(*length).ok_or_else(invalid)?;
    if count_type
        != ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?)
    {
        return Err(invalid());
    }
    let mut source_pointer = byte_views::backing_pointer(builder, row, *source)?;
    // A subslice retains root backing plus a view offset. Its own first byte,
    // rather than the backing's first byte, is the copy's source.
    if let Some(view) = builder
        .transport
        .views
        .iter()
        .find(|view| view.place == *source)
        .copied()
    {
        let address = transport_register(builder, *source, 0)?;
        builder.emit(
            SelectedInstructionKind::ByteViewAddress,
            builder.constraints.keys.add_i64,
            &[source_pointer, view.byte_offset, address],
            provenance(row),
        )?;
        source_pointer = address;
    }
    let root = builder
        .transport
        .pointers
        .iter()
        .find(|(place, _)| *place == destination.place)
        .map(|(_, pointer)| *pointer)
        .ok_or_else(invalid)?;
    let destination_pointer = transport_register(builder, destination.place, byte_offset)?;
    builder.emit(
        SelectedInstructionKind::AddressOffset { byte_offset },
        builder
            .constraints
            .keys
            .address_offset
            .ok_or_else(invalid)?,
        &[root, destination_pointer],
        provenance(row),
    )?;
    let cursor = scratch(builder, 3)?;
    let byte = scratch(builder, 4)?;
    // The copy's source bytes live in the view's storage root, not the view's
    // own identity; a block-parameter view reaches each bound root.
    match crate::selection::byte_view_homes::view_backing_roots(
        function,
        &builder.transport.views,
        *source,
        *length,
    )? {
        Some(roots) => {
            for (root, extent) in roots {
                memory(
                    builder,
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
            builder,
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
        builder,
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
    builder.emit(
        SelectedInstructionKind::CopyBytes,
        builder.constraints.keys.copy_bytes.ok_or_else(invalid)?,
        &[source_pointer, destination_pointer, count, cursor, byte],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![*length],
            obligations: vec![*obligation],
            ..Default::default()
        },
    )?;
    memory(
        builder,
        row,
        destination.place,
        metadata_offset,
        8,
        SelectedMemoryAccessRole::WritePlace,
    )?;
    builder.emit(
        SelectedInstructionKind::Store {
            byte_offset: metadata_offset,
            byte_size: 8,
        },
        builder.constraints.keys.store.ok_or_else(invalid)?,
        &[root, count],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![*length],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )
}

pub(super) fn replace_byte(
    function: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
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
        return Err(invalid());
    };
    let signature = function.structural.as_ref().ok_or_else(invalid)?;
    let parameter = signature
        .parameters
        .iter()
        .find(|parameter| parameter.semantic.place == destination.place)
        .ok_or_else(invalid)?;
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
        return Err(invalid());
    }
    let (metadata_offset, _) =
        crate::structural_inputs::structural_reference_input::byte_field_storage(
            parameter.semantic.structural_type,
            &destination.path,
            *field,
            &signature.structural_types,
        )
        .ok_or_else(invalid)?;
    let payload_offset = metadata_offset.checked_add(8).ok_or_else(invalid)?;
    let (_, index_register, _, index_type) = builder.resolve(*index).ok_or_else(invalid)?;
    let (_, value_register, _, value_type) = builder.resolve(*value).ok_or_else(invalid)?;
    let (_, _, _, length_type) = builder.resolve(*length).ok_or_else(invalid)?;
    let count_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?);
    if index_type != count_type
        || length_type != count_type
        || value_type
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
        .find(|(place, _)| *place == destination.place)
        .map(|(_, pointer)| *pointer)
        .ok_or_else(invalid)?;
    let payload = transport_register(builder, destination.place, payload_offset)?;
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
    let address = transport_register(builder, destination.place, payload_offset)?;
    builder.emit(
        SelectedInstructionKind::ByteViewAddress,
        builder.constraints.keys.add_i64,
        &[payload, index_register, address],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![*index, *length],
            obligations: vec![*obligation],
            ..Default::default()
        },
    )?;
    memory(
        builder,
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
    // Metadata and displaced content are not loaded. Only this committing byte
    // store carries fuel; the original inline backing and live length survive.
    builder.emit(
        SelectedInstructionKind::Store {
            byte_offset: 0,
            byte_size: 1,
        },
        builder.constraints.keys.store.ok_or_else(invalid)?,
        &[address, value_register],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![*index, *value, *length],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )
}

fn scratch(
    builder: &mut Builder<'_>,
    operand: u16,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let id = VirtualRegisterId(builder.registers.len().try_into().map_err(|_| invalid())?);
    let instruction = SelectedInstructionId(
        builder
            .instructions
            .len()
            .try_into()
            .map_err(|_| invalid())?,
    );
    builder.registers.push(VirtualRegister {
        id,
        scalar_type: ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?,
        ),
        class: builder.class,
        origin: VirtualRegisterOrigin::InstructionScratch {
            instruction,
            operand,
        },
        definition_site: None,
        entry_fixed_view: None,
    });
    Ok(id)
}
