//! Whole replacement copies live bytes before publishing the new length;
//! indexed replacement addresses one byte in that same original inline backing.
//! Capacity never supplies readable source extent; unused destination bytes are
//! not observed or cleared. ISA-local scratch/control is not source control flow.
use super::{
    Builder, IntegerSign, IntegerType, LegalizedScalarFunction, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind, ScalarType, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedMemoryAccessRole, StructuralAccess, VirtualRegisterId,
    memory,
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
    let signature = function.structural.as_ref().ok_or_else(|| invalid())?;
    let (destination, field, source, length, obligation, accepted_fact) = match &row.kind {
        LegalizedScalarInstructionKind::ByteSequenceStore {
            destination,
            source,
            length,
            obligation,
            accepted_fact,
        } => {
            let access = if let Some(parameter) =
                crate::selection::primitive_local_input::read_parameter(function, *destination)
            {
                if parameter.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                    || !parameter.projected_qualifications.is_empty()
                {
                    return Err(invalid());
                }
                parameter.access
            } else {
                let (_, home) =
                    crate::selection::aggregate_result_input::home(function, *destination)
                        .ok_or_else(|| invalid())?;
                if home.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted {
                    return Err(invalid());
                }
                StructuralAccess::Owned
            };
            if !matches!(
                access,
                StructuralAccess::Owned
                    | StructuralAccess::MutableBorrow
                    | StructuralAccess::WriteOnlyBorrow
            ) {
                return Err(invalid());
            }
            (
                terminal_psi::StructuralArgument {
                    place: *destination,
                    access,
                    path: Vec::new(),
                },
                None,
                source,
                length,
                obligation,
                accepted_fact,
            )
        }
        LegalizedScalarInstructionKind::StructuralByteSequenceFieldStore {
            destination,
            field,
            source,
            length,
            obligation,
            accepted_fact,
        } => (
            destination.clone(),
            Some(*field),
            source,
            length,
            obligation,
            accepted_fact,
        ),
        _ => return Err(invalid()),
    };
    let reference = if field.is_some() && destination.access == StructuralAccess::MutableBorrow {
        crate::selection::reference_input::referent_projection(
            function,
            destination.place,
            &destination.path,
            StructuralAccess::MutableBorrow,
        )
    } else {
        None
    };
    let destination_type = if let Some((referent, _)) = reference {
        referent
    } else if destination.access == StructuralAccess::Owned {
        // The actual home retains its type across operation production and
        // state arrival. Field geometry below selects inside that home; its
        // enclosing aggregate need not itself be a record.
        if let Some(parameter) =
            crate::selection::primitive_local_input::read_parameter(function, destination.place)
        {
            if parameter.access != StructuralAccess::Owned {
                return Err(invalid());
            }
            parameter.structural_type
        } else {
            crate::selection::aggregate_result_input::home(function, destination.place)
                .ok_or_else(|| invalid())?
                .1
                .structural_type
        }
    } else {
        let parameter = crate::selection::address_join_input::loan_declaration(
            function,
            destination.place,
            destination.access,
        )
        .ok_or_else(|| invalid())?;
        if parameter.access != destination.access
            || !matches!(
                destination.access,
                terminal_psi::exclusive_borrow_pair_pattern!()
            )
            || parameter.multiplicity == terminal_psi::StructuralMultiplicity::Linear
            || (field.is_some() && !parameter.qualifications.is_empty())
            || !parameter.projected_qualifications.is_empty()
        {
            return Err(invalid());
        }
        parameter.structural_type
    };
    if row.result.is_some()
        || *source == destination.place
        || crate::selection::established_view_input::readable_byte_type(function, *source).is_none()
    {
        return Err(invalid());
    }
    let metadata_offset = if let Some(field) = field {
        crate::structural_inputs::structural_reference_input::byte_field_storage(
            destination_type,
            reference.map_or(destination.path.as_slice(), |(_, suffix)| suffix),
            field,
            &signature.structural_types,
        )
        .ok_or_else(|| invalid())?
        .0
    } else {
        if !signature.structural_types.iter().any(|declaration| {
            declaration.id == destination_type && declaration.shape.is_bounded_owned_byte_sequence()
        }) {
            return Err(invalid());
        }
        0
    };
    let byte_offset = metadata_offset.checked_add(8).ok_or_else(|| invalid())?;
    let (_, count, _, count_type) = builder.resolve(*length).ok_or_else(|| invalid())?;
    if count_type
        != ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?)
    {
        return Err(invalid());
    }
    let mut source_pointer = byte_views::backing_pointer(function, builder, row, *source)?;
    // A subslice retains root backing plus a view offset. Its own first byte,
    // rather than the backing's first byte, is the copy's source.
    if let Some(view) = builder.transport.view(*source).copied() {
        let address = transport_register(builder, *source, 0)?;
        builder.emit_byte_view_address(
            source_pointer,
            view.byte_offset,
            address,
            provenance(row),
        )?;
        source_pointer = address;
    }
    let root = if reference.is_some() {
        let home = super::place_pointer(builder, row, destination.place, 0)?;
        super::reference::load(builder, row, destination.place, home)?
    } else if destination.access == StructuralAccess::Owned {
        // Borrow the completed record's actual home; no incoming ABI pointer
        // or fabricated parameter represents this local value.
        super::place_pointer(builder, row, destination.place, 0)?
    } else {
        builder
            .transport
            .pointers
            .iter()
            .find(|(place, _)| *place == destination.place)
            .map(|(_, pointer)| *pointer)
            .ok_or_else(|| invalid())?
    };
    let destination_pointer = transport_register(builder, destination.place, byte_offset)?;
    builder.emit_address_offset(byte_offset, root, destination_pointer, provenance(row))?;
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
            if crate::selection::byte_concatenation_input::owner_layout_type(function, *source)
                .is_some()
            {
                8
            } else {
                0
            },
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
        builder
            .constraints
            .keys
            .copy_bytes
            .ok_or_else(|| invalid())?,
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
    builder.emit_store(
        metadata_offset,
        8,
        root,
        count,
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
    let signature = function.structural.as_ref().ok_or_else(|| invalid())?;
    let parameter = crate::selection::address_join_input::loan_declaration(
        function,
        destination.place,
        destination.access,
    )
    .ok_or_else(|| invalid())?;
    if row.result.is_some()
        || parameter.access != destination.access
        || !matches!(
            destination.access,
            terminal_psi::exclusive_borrow_pair_pattern!()
        )
        || parameter.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
    {
        return Err(invalid());
    }
    let (metadata_offset, _) =
        crate::structural_inputs::structural_reference_input::byte_field_storage(
            parameter.structural_type,
            &destination.path,
            *field,
            &signature.structural_types,
        )
        .ok_or_else(|| invalid())?;
    let payload_offset = metadata_offset.checked_add(8).ok_or_else(|| invalid())?;
    let (_, index_register, _, index_type) = builder.resolve(*index).ok_or_else(|| invalid())?;
    let (_, value_register, _, value_type) = builder.resolve(*value).ok_or_else(|| invalid())?;
    let (_, _, _, length_type) = builder.resolve(*length).ok_or_else(|| invalid())?;
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
        .ok_or_else(|| invalid())?;
    let payload = transport_register(builder, destination.place, payload_offset)?;
    builder.emit_address_offset(payload_offset, root, payload, provenance(row))?;
    let address = transport_register(builder, destination.place, payload_offset)?;
    builder.emit_byte_view_address(
        payload,
        index_register,
        address,
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
    builder.emit_store(
        0,
        1,
        address,
        value_register,
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![*index, *value, *length],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )
}

pub(super) fn scratch(
    builder: &mut Builder<'_>,
    operand: u16,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    super::super::integer_arithmetic::instruction_scratch(builder, operand, IntegerSign::Unsigned)
}
