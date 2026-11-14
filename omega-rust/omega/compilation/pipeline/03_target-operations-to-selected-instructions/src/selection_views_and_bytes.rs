/*
We select byte-sequence and element-view operations for structural instruction
selection, including byte concatenation, copying, field reads/stores, subslices,
element-view establishment, and element writes/observations.
*/

pub(crate) mod byte_concatenation {
    use crate::SelectedInstructionError;
    use crate::selection::byte_concatenation_input::field_offset;
    use crate::selection::construction::scalar_graph::structural::byte_field_read::payload_pointer;
    use crate::selection::construction::scalar_graph::structural::{
        Builder, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
        SelectedInstructionKind, SelectedInstructionProvenance, SelectedMemoryAccessRole,
        VirtualRegisterId, memory,
    };
    use crate::selection::construction::scalar_graph::structural::{
        byte_field_store, byte_views, invalid, local_storage, obligation_provenance, provenance,
        transport_register,
    };
    use semantic_vocabulary::{IntegerSign, IntegerType, PlaceId, ScalarType, ValueId};
    use terminal_psi::control_flow::ByteSequenceOperand;

    pub(crate) fn establish(
        function: &crate::native_geometry::NativeFunction<'_>,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
    ) -> Result<(), SelectedInstructionError> {
        let LegalizedScalarInstructionKind::EstablishByteSequenceConcatenation {
            result,
            left,
            right,
            capacity_obligation,
            accepted_fact,
        } = &row.kind
        else {
            return Err(invalid());
        };
        let capacity =
            crate::selection::byte_concatenation_input::capacity(function, row).ok_or(invalid())?;
        if builder
            .transport
            .pointers
            .iter()
            .any(|(place, _)| *place == result.place)
        {
            return Err(invalid());
        }
        let integer = ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?,
        );
        let (_, left_length, _, left_type) = builder.resolve(left.length()).ok_or(invalid())?;
        let (_, right_length, _, right_type) = builder.resolve(right.length()).ok_or(invalid())?;
        if left_type != integer || right_type != integer {
            return Err(invalid());
        }
        let left_pointer = operand_pointer(function, builder, row, left)?;
        let right_pointer = operand_pointer(function, builder, row, right)?;
        let slot = builder.transport.push_structural_local_slot(
            row.operation,
            result.place,
            capacity.checked_add(8).ok_or(invalid())?,
            8,
        );
        let owner = local_storage::address(builder, row, slot, 0, capacity + 8, false)?;
        let payload = transport_register(builder, result.place, 8)?;
        builder.emit_address_offset(8, owner, payload, provenance(row))?;
        copy(
            function,
            builder,
            row,
            left,
            left_pointer,
            payload,
            left_length,
            capacity,
        )?;
        let right_destination = transport_register(builder, result.place, 8)?;
        builder.emit_byte_view_address(payload, left_length, right_destination, provenance(row))?;
        copy(
            function,
            builder,
            row,
            right,
            right_pointer,
            right_destination,
            right_length,
            capacity,
        )?;
        let length = transport_register(builder, result.place, 0)?;
        let evidence = obligation_provenance(
            row,
            &[left.length(), right.length()],
            &[*capacity_obligation],
        );
        builder.emit(
            SelectedInstructionKind::ExactAddI64 {
                obligation: *capacity_obligation,
                accepted_fact: *accepted_fact,
            },
            builder.constraints.keys.add_i64,
            &[left_length, right_length, length],
            evidence.clone(),
        )?;
        memory(
            builder,
            row,
            result.place,
            0,
            8,
            SelectedMemoryAccessRole::WritePlace,
        )?;
        builder.emit_store(
            0,
            8,
            owner,
            length,
            SelectedInstructionProvenance {
                work: row.work.clone(),
                ..evidence
            },
        )?;
        builder.transport.pointers.push((result.place, owner));
        Ok(())
    }

    pub(crate) fn operand_pointer(
        function: &crate::native_geometry::NativeFunction<'_>,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        operand: &ByteSequenceOperand,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        match operand {
            ByteSequenceOperand::Whole { source, .. } => {
                if !source.path.is_empty() {
                    return crate::selection::construction::scalar_graph::structural::descriptor_operands::backing(function, builder, row, source, &[]);
                }
                let pointer = byte_views::backing_pointer(function, builder, row, source.place)?;
                if let Some(view) = builder.transport.view(source.place).copied() {
                    let address = transport_register(builder, source.place, 0)?;
                    builder.emit_byte_view_address(
                        pointer,
                        view.byte_offset,
                        address,
                        provenance(row),
                    )?;
                    Ok(address)
                } else {
                    Ok(pointer)
                }
            }
            ByteSequenceOperand::Field { source, field, .. } => {
                let geometry = crate::selection::field_bytes::FieldBytes::resolve(
                    function,
                    source,
                    *field,
                    &[],
                )
                .ok_or(invalid())?;
                payload_pointer(function, builder, row, source.place, geometry, &[])
            }
        }
    }

    pub(crate) fn operand_span(
        function: &crate::native_geometry::NativeFunction<'_>,
        builder: &Builder<'_>,
        operand: &ByteSequenceOperand,
    ) -> Result<(u32, Vec<(PlaceId, ValueId)>), SelectedInstructionError> {
        let offset = match operand {
            ByteSequenceOperand::Field { .. } => field_offset(function, operand).ok_or(invalid())?,
            ByteSequenceOperand::Whole { source, .. } => {
                let (geometry, shape) =
                    crate::selection::construction::scalar_graph::structural::descriptor_operands::source_geometry(function, source).ok_or(invalid())?;
                if shape.is_bounded_owned_byte_sequence() {
                    geometry.byte_offset.checked_add(8).ok_or(invalid())?
                } else {
                    geometry.byte_offset
                }
            }
        };
        let roots = match operand {
            ByteSequenceOperand::Whole { source, length } if source.path.is_empty() => {
                crate::selection::byte_view_homes::view_backing_roots(
                    function,
                    &builder.transport.views,
                    source.place,
                    *length,
                )?
            }
            ByteSequenceOperand::Whole { .. } | ByteSequenceOperand::Field { .. } => None,
        };
        Ok((
            offset,
            roots.unwrap_or_else(|| vec![(operand.source(), operand.length())]),
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn copy(
        function: &crate::native_geometry::NativeFunction<'_>,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        operand: &ByteSequenceOperand,
        source_pointer: VirtualRegisterId,
        destination_pointer: VirtualRegisterId,
        count: VirtualRegisterId,
        capacity: u32,
    ) -> Result<(), SelectedInstructionError> {
        let LegalizedScalarInstructionKind::EstablishByteSequenceConcatenation {
            result,
            capacity_obligation,
            accepted_fact,
            ..
        } = &row.kind
        else {
            return Err(invalid());
        };
        let cursor = byte_field_store::scratch(builder, 3)?;
        let byte = byte_field_store::scratch(builder, 4)?;
        let (offset, roots) = operand_span(function, builder, operand)?;
        for (root, length) in roots {
            memory(
                builder,
                row,
                root,
                offset,
                0,
                SelectedMemoryAccessRole::ReadByteSpan {
                    length,
                    obligation: *capacity_obligation,
                    accepted_fact: *accepted_fact,
                },
            )?;
        }
        memory(
            builder,
            row,
            result.place,
            8,
            capacity,
            SelectedMemoryAccessRole::WritePlace,
        )?;
        builder.emit(
            SelectedInstructionKind::CopyBytes,
            builder.constraints.keys.copy_bytes.ok_or(invalid())?,
            &[source_pointer, destination_pointer, count, cursor, byte],
            obligation_provenance(row, &[operand.length()], &[*capacity_obligation]),
        )
    }
}

pub(crate) mod byte_copy {
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::runtime_address::{
        Written, footprint_at_origin,
    };
    use crate::selection::construction::scalar_graph::structural::{
        Builder, SelectedInstructionKind, SelectedInstructionProvenance, transport_register,
    };
    use crate::selection::extent_copy;
    use crate::selection::scalar_call_abi::load_transfer;
    use selected_instructions::{SelectedMemoryAccessOrigin, VirtualRegisterId};
    use semantic_vocabulary::{IntegerValue, PlaceId};

    pub(crate) struct CopyEnd<'a> {
        pub(crate) place: PlaceId,
        pub(crate) pointer: VirtualRegisterId,
        pub(crate) byte_offset: u32,
        pub(crate) elements: &'a [legalized_operations::LegalizedRuntimeIndexOperand],
    }

    pub(crate) fn copy(
        builder: &mut Builder<'_>,
        origin: SelectedMemoryAccessOrigin,
        provenance: SelectedInstructionProvenance,
        from: CopyEnd<'_>,
        to: CopyEnd<'_>,
        byte_size: u32,
    ) -> Result<(), SelectedInstructionError> {
        if byte_size > extent_copy::INLINE_MAX_BYTES {
            let source = address(builder, &from, provenance.clone())?;
            let destination = address(builder, &to, provenance.clone())?;
            let count = transport_register(builder, to.place, 0)?;
            builder.emit_materialize_i64(
                IntegerValue::Unsigned(u128::from(byte_size)),
                count,
                provenance.clone(),
            )?;
            let key = builder.constraints.keys.copy_bytes.ok_or_else(|| {
                SelectedInstructionError::custody_because(
                    "exact extent copy has no admitted byte-copy form",
                )
            })?;
            let instruction = selected_instructions::SelectedInstructionId(
                builder
                    .instructions
                    .len()
                    .try_into()
                    .map_err(|_| SelectedInstructionError::custody())?,
            );
            let operands = extent_copy::bulk_operands(
                instruction,
                [source, destination, count],
                key,
                builder.catalog,
                &mut builder.registers,
            )?;
            access(builder, origin, &from, 0, byte_size, Written::None)?;
            access(builder, origin, &to, 0, byte_size, Written::Structural)?;
            return builder.emit(
                SelectedInstructionKind::CopyBytes,
                key,
                &operands,
                provenance,
            );
        }
        for (cursor, width) in extent_copy::chunks(byte_size) {
            let offset = from
                .byte_offset
                .checked_add(cursor)
                .ok_or_else(SelectedInstructionError::custody)?;
            let (input, displacement) =
                crate::selection::construction::scalar_graph::structural::displaced_memory_pointer(
                    builder,
                    from.place,
                    from.pointer,
                    offset,
                    u32::from(width),
                    provenance.clone(),
                )?;
            let value = transport_register(builder, from.place, offset)?;
            access(
                builder,
                origin,
                &from,
                cursor,
                u32::from(width),
                Written::None,
            )?;
            let (kind, key) =
                load_transfer(&builder.constraints.keys, u32::from(width), displacement)
                    .ok_or_else(SelectedInstructionError::custody)?;
            builder.emit(
                kind,
                key.ok_or_else(SelectedInstructionError::custody)?,
                &[input, value],
                provenance.clone(),
            )?;
            let offset = to
                .byte_offset
                .checked_add(cursor)
                .ok_or_else(SelectedInstructionError::custody)?;
            let (output, displacement) =
                crate::selection::construction::scalar_graph::structural::displaced_memory_pointer(
                    builder,
                    to.place,
                    to.pointer,
                    offset,
                    u32::from(width),
                    provenance.clone(),
                )?;
            access(
                builder,
                origin,
                &to,
                cursor,
                u32::from(width),
                Written::Structural,
            )?;
            builder.emit_store(displacement, width, output, value, provenance.clone())?;
        }
        Ok(())
    }

    fn address(
        builder: &mut Builder<'_>,
        end: &CopyEnd<'_>,
        provenance: SelectedInstructionProvenance,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        if end.byte_offset == 0 {
            return Ok(end.pointer);
        }
        let address = transport_register(builder, end.place, end.byte_offset)?;
        builder.emit_address_offset(end.byte_offset, end.pointer, address, provenance)?;
        Ok(address)
    }

    fn access(
        builder: &mut Builder<'_>,
        origin: SelectedMemoryAccessOrigin,
        end: &CopyEnd<'_>,
        cursor: u32,
        byte_size: u32,
        written: Written,
    ) -> Result<(), SelectedInstructionError> {
        let offset = end
            .byte_offset
            .checked_add(cursor)
            .ok_or_else(SelectedInstructionError::custody)?;
        footprint_at_origin(
            builder,
            origin,
            end.place,
            offset,
            byte_size,
            end.elements,
            written,
        )
    }
}

pub(crate) mod byte_field_read {
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::invalid;
    use crate::selection::construction::scalar_graph::structural::operation_provenance;
    use crate::selection::construction::scalar_graph::structural::provenance;
    use crate::selection::construction::scalar_graph::structural::runtime_address::{
        cross_references, footprint, projection_origin, scale,
    };
    use crate::selection::construction::scalar_graph::structural::transport_register;
    use crate::selection::construction::scalar_graph::structural::{
        Builder, IntegerSign, IntegerType, LegalizedScalarInstruction,
        LegalizedScalarInstructionKind, ScalarType, SelectedInstructionKind,
        SelectedMemoryAccessRole, VirtualRegisterId, memory,
    };

    pub(crate) fn read_byte(
        function: &crate::native_geometry::NativeFunction<'_>,
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
        let definition = row.result.ok_or(invalid())?;
        let geometry =
            crate::selection::field_bytes::FieldBytes::resolve(function, source, *field, indices)
                .ok_or_else(|| invalid())?;
        let payload_offset = geometry.offset();
        let (_, index_register, _, index_type) = builder.resolve(*index).ok_or(invalid())?;
        let (_, _, _, length_type) = builder.resolve(*length).ok_or(invalid())?;
        let count_type = ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?,
        );
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
            builder.constraints.keys.load8_indexed.ok_or(invalid())?,
            &[payload, index_register, output],
            operation_provenance(row, vec![*index, *length, definition.value]),
        )?;
        Ok(output)
    }

    pub(crate) fn payload_pointer(
        function: &crate::native_geometry::NativeFunction<'_>,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        place: semantic_vocabulary::PlaceId,
        geometry: crate::selection::field_bytes::FieldBytes,
        indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let offset = geometry.offset();
        let (root, pointer_slots) = projection_origin(
            function,
            builder,
            row,
            place,
            offset,
            &geometry.pointer_slots,
        )?;
        let root = root.ok_or_else(invalid)?;
        let (root, first_leaf_index) =
            cross_references(builder, row, place, root, indices, pointer_slots)?;
        let indices = indices.get(first_leaf_index..).ok_or_else(|| invalid())?;
        let root = scale(builder, row, place, offset, root, indices)?;
        let payload = transport_register(builder, place, offset)?;
        if !geometry.is_descriptor {
            builder.emit_address_offset(offset, root, payload, provenance(row))?;
        } else {
            footprint(
                builder,
                row,
                place,
                offset,
                8,
                indices,
                crate::selection::construction::scalar_graph::structural::runtime_address::Written::None,
            )?;
            builder.emit_load64(offset, root, payload, provenance(row))?;
        }
        Ok(payload)
    }
}

pub(crate) mod byte_field_store {
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::byte_views;
    use crate::selection::construction::scalar_graph::structural::invalid;
    use crate::selection::construction::scalar_graph::structural::obligation_provenance;
    use crate::selection::construction::scalar_graph::structural::operation_provenance;
    use crate::selection::construction::scalar_graph::structural::provenance;
    use crate::selection::construction::scalar_graph::structural::transport_register;
    use crate::selection::construction::scalar_graph::structural::{
        Builder, IntegerSign, IntegerType, LegalizedScalarInstruction,
        LegalizedScalarInstructionKind, ScalarType, SelectedInstructionKind,
        SelectedMemoryAccessRole, StructuralAccess, VirtualRegisterId, memory,
    };
    use crate::selection::primitive_local_input::read_parameter;

    pub(crate) fn replace(
        function: &crate::native_geometry::NativeFunction<'_>,
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
                let access = if let Some(parameter) = read_parameter(function, *destination) {
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
        let reference = if field.is_some() && destination.access == StructuralAccess::MutableBorrow
        {
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
            if let Some(parameter) = read_parameter(function, destination.place) {
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
            || crate::selection::established_view_input::readable_byte_type(function, *source)
                .is_none()
        {
            return Err(invalid());
        }
        let metadata_offset = if let Some(field) = field {
            crate::structural_inputs::structural_reference_input::byte_field_storage(
                destination_type,
                reference.map_or(destination.path.as_slice(), |(_, suffix)| suffix),
                field,
                function.geometry(),
            )
            .ok_or_else(|| invalid())?
            .0
        } else {
            if !signature.structural_types.iter().any(|declaration| {
                declaration.id == destination_type
                    && declaration.shape.is_bounded_owned_byte_sequence()
            }) {
                return Err(invalid());
            }
            0
        };
        let byte_offset = metadata_offset.checked_add(8).ok_or_else(|| invalid())?;
        let (_, count, _, count_type) = builder.resolve(*length).ok_or_else(|| invalid())?;
        if count_type
            != ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?,
            )
        {
            return Err(invalid());
        }
        let mut source_pointer = byte_views::backing_pointer(function, builder, row, *source)?;
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
            let home = crate::selection::construction::scalar_graph::structural::place_pointer(
                builder,
                row,
                destination.place,
                0,
            )?;
            crate::selection::construction::scalar_graph::structural::reference::load(
                builder,
                row,
                destination.place,
                home,
            )?
        } else if destination.access == StructuralAccess::Owned {
            crate::selection::construction::scalar_graph::structural::place_pointer(
                builder,
                row,
                destination.place,
                0,
            )?
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
        byte_views::charge_backing_roots(
            function,
            builder,
            row,
            *source,
            *length,
            |extent| SelectedMemoryAccessRole::ReadByteSpan {
                length: extent,
                obligation: *obligation,
                accepted_fact: *accepted_fact,
            },
            byte_views::unsliced_payload_offset(function, *source),
            0,
            SelectedMemoryAccessRole::ReadByteSpan {
                length: *length,
                obligation: *obligation,
                accepted_fact: *accepted_fact,
            },
        )?;
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
            obligation_provenance(row, &[*length], &[*obligation]),
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
            operation_provenance(row, vec![*length]),
        )
    }

    pub(crate) fn replace_byte(
        function: &crate::native_geometry::NativeFunction<'_>,
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
        function.structural.as_ref().ok_or_else(|| invalid())?;
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
                function.geometry(),
            )
            .ok_or_else(|| invalid())?;
        let payload_offset = metadata_offset.checked_add(8).ok_or_else(|| invalid())?;
        let (_, index_register, _, index_type) =
            builder.resolve(*index).ok_or_else(|| invalid())?;
        let (_, value_register, _, value_type) =
            builder.resolve(*value).ok_or_else(|| invalid())?;
        let (_, _, _, length_type) = builder.resolve(*length).ok_or_else(|| invalid())?;
        let count_type = ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?,
        );
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
            obligation_provenance(row, &[*index, *length], &[*obligation]),
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
        builder.emit_store(
            0,
            1,
            address,
            value_register,
            operation_provenance(row, vec![*index, *value, *length]),
        )
    }

    pub(crate) fn scratch(
        builder: &mut Builder<'_>,
        operand: u16,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        crate::selection::construction::scalar_graph::integer_arithmetic::instruction_scratch(
            builder,
            operand,
            IntegerSign::Unsigned,
        )
    }
}

pub(crate) mod byte_view_field_store {
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::invalid;
    use crate::selection::construction::scalar_graph::structural::provenance;
    use crate::selection::construction::scalar_graph::structural::runtime_address::{
        footprint, projection_base, scale,
    };
    use crate::selection::construction::scalar_graph::structural::transport_register;
    use crate::selection::construction::scalar_graph::structural::{
        Builder, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
        SelectedInstructionProvenance, SelectedMemoryAccessRole, StructuralAccess, memory,
    };
    use crate::selection::established_view_input::{descriptor_home, view_type};

    pub(crate) fn replace(
        function: &crate::native_geometry::NativeFunction<'_>,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
    ) -> Result<(), SelectedInstructionError> {
        let LegalizedScalarInstructionKind::StructuralByteViewFieldStore {
            destination,
            field,
            source,
            indices,
        } = &row.kind
        else {
            return Err(invalid());
        };
        let signature = function.structural.as_ref().ok_or(invalid())?;
        let root_type = match field {
            Some(_) => crate::selection::field_bytes::root_type(function, destination),
            None => descriptor_home(function, destination.place)
                .filter(|_| destination.access == StructuralAccess::Owned)
                .map(|(_, home)| home.structural_type),
        }
        .ok_or_else(|| invalid())?;
        let geometry = crate::structural_inputs::structural_reference_input::projected_view_slot(
            root_type,
            &destination.path,
            *field,
            function.geometry(),
        )
        .ok_or_else(|| invalid())?;
        if row.result.is_some()
            || !matches!(
                destination.access,
                StructuralAccess::Owned
                    | StructuralAccess::MutableBorrow
                    | StructuralAccess::WriteOnlyBorrow
            )
            || *source == destination.place
            || !view_type(function, *source).is_some_and(|identity| {
                signature
                    .structural_types
                    .iter()
                    .any(|row| row.id == identity && row.shape.is_borrowed_byte_view())
            })
            || !crate::structural_inputs::runtime_indices_match(indices, &geometry.elements)
        {
            return Err(invalid());
        }
        let descriptor_offset = geometry.descriptor_offset.ok_or_else(|| invalid())?;
        let extent_offset = descriptor_offset.checked_add(8).ok_or(invalid())?;
        let descriptor = builder
            .transport
            .pointers
            .iter()
            .find(|(place, _)| place == source)
            .map(|(_, pointer)| *pointer)
            .ok_or(invalid())?;
        let root = match field {
            Some(_) => {
                projection_base(function, builder, row, destination.place, descriptor_offset)?
                    .ok_or_else(|| invalid())?
            }
            None => builder
                .transport
                .pointers
                .iter()
                .find(|(place, _)| *place == destination.place)
                .map(|(_, pointer)| *pointer)
                .ok_or_else(|| invalid())?,
        };
        let root = scale(
            builder,
            row,
            destination.place,
            descriptor_offset,
            root,
            indices,
        )?;
        let backing = transport_register(builder, *source, 0)?;
        memory(
            builder,
            row,
            *source,
            0,
            8,
            SelectedMemoryAccessRole::ReadPlace,
        )?;
        builder.emit_load64(0, descriptor, backing, provenance(row))?;
        let extent = transport_register(builder, *source, 8)?;
        memory(
            builder,
            row,
            *source,
            8,
            8,
            SelectedMemoryAccessRole::ReadPlace,
        )?;
        builder.emit_load64(8, descriptor, extent, provenance(row))?;
        footprint(
            builder,
            row,
            destination.place,
            descriptor_offset,
            8,
            indices,
            crate::selection::construction::scalar_graph::structural::runtime_address::Written::Structural,
        )?;
        builder.emit_store(descriptor_offset, 8, root, backing, provenance(row))?;
        footprint(
            builder,
            row,
            destination.place,
            extent_offset,
            8,
            indices,
            crate::selection::construction::scalar_graph::structural::runtime_address::Written::Structural,
        )?;
        builder.emit_store(
            extent_offset,
            8,
            root,
            extent,
            SelectedInstructionProvenance {
                operations: vec![row.operation],
                work: row.work.clone(),
                ..Default::default()
            },
        )
    }
}

pub(crate) mod byte_views {
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::invalid;
    use crate::selection::construction::scalar_graph::structural::operation_provenance;
    use crate::selection::construction::scalar_graph::structural::provenance;
    use crate::selection::construction::scalar_graph::structural::transport_register;
    use crate::selection::construction::scalar_graph::structural::{
        Builder, IntegerSign, IntegerType, LegalizedScalarInstruction,
        LegalizedScalarInstructionKind, PlaceId, ScalarType, SelectedInstructionKind,
        SelectedMemoryAccessRole, ValueId, VirtualRegisterId, memory, obligation_provenance,
    };
    use crate::selection::construction::scalar_graph::structural::{
        byte_concatenation, byte_field_store,
    };
    use crate::selection::established_view_input::readable_byte_type;
    use terminal_psi::control_flow::ByteSequenceOperand;

    pub(crate) fn write(
        function: &crate::native_geometry::NativeFunction<'_>,
        builder: &mut Builder<'_>,
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
            return Err(invalid());
        };
        let (_, index_register, _, index_type) = builder.resolve(index).ok_or_else(|| invalid())?;
        let (_, value_register, _, value_type) = builder.resolve(value).ok_or_else(|| invalid())?;
        if readable_byte_type(function, destination).is_none()
            || row.result.is_some()
            || index_type
                != ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?,
                )
            || value_type
                != ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| invalid())?,
                )
        {
            return Err(invalid());
        }
        let pointer = backing_pointer(function, builder, row, destination)?;
        let physical_index = backing_index(builder, row, destination, index_register)?;
        let address = transport_register(builder, destination, 0)?;
        builder.emit_byte_view_address(
            pointer,
            physical_index,
            address,
            obligation_provenance(row, &[index, length], &[obligation]),
        )?;
        charge_backing_roots(
            function,
            builder,
            row,
            destination,
            length,
            |extent| SelectedMemoryAccessRole::WriteByteSpan {
                length: extent,
                obligation,
                accepted_fact,
            },
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
        builder.emit_store(
            0,
            1,
            address,
            value_register,
            operation_provenance(row, vec![index, value, length]),
        )
    }

    pub(crate) fn byte_observation(
        function: &crate::native_geometry::NativeFunction<'_>,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        match &row.kind {
            LegalizedScalarInstructionKind::ByteSequenceRead { source, .. } => {
                if readable_byte_type(function, *source).is_none() {
                    return Err(invalid());
                }
                byte_sequence_read(function, builder, row)
            }
            LegalizedScalarInstructionKind::ByteSequenceLength {
                source,
                length_byte_offset,
                ..
            } => {
                let (_, terminal_psi::StructuralTypeShape::ByteSequence(carrier)) =
                    crate::selection::construction::scalar_graph::structural::descriptor_operands::source_geometry(function, source).ok_or(invalid())?
                else {
                    return Err(invalid());
                };
                if let terminal_psi::ByteSequenceCarrier::BoundedView { capacity, .. } = carrier {
                    let result = row.result.ok_or(invalid())?;
                    let output = builder.register(
                        result.value,
                        result.definition_site,
                        result.scalar_type,
                    )?;
                    builder.emit_materialize_i64(
                        semantic_vocabulary::IntegerValue::Unsigned(u128::from(capacity)),
                        output,
                        provenance(row),
                    )?;
                    return Ok(output);
                }
                let expected = if matches!(
                    carrier,
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                ) {
                    0
                } else {
                    8
                };
                if *length_byte_offset != expected {
                    return Err(invalid());
                }
                if source.path.is_empty() && builder.transport.view(source.place).is_some() {
                    byte_sequence_length(builder, row, source.place, *length_byte_offset)
                } else {
                    crate::selection::construction::scalar_graph::structural::primitive_locals::read(
                        function, builder, row,
                    )
                }
            }
            _ => Err(invalid()),
        }
    }

    fn byte_sequence_read(
        function: &crate::native_geometry::NativeFunction<'_>,
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
        let definition = row.result.ok_or_else(|| invalid())?;
        let (_, index_register, _, index_type) = builder.resolve(index).ok_or_else(|| invalid())?;
        if definition.scalar_type
            != ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| invalid())?,
            )
            || index_type
                != ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?,
                )
        {
            return Err(invalid());
        }
        let pointer = backing_pointer(function, builder, row, source)?;
        let physical_index = backing_index(builder, row, source, index_register)?;
        let output = builder.register(
            definition.value,
            definition.definition_site,
            definition.scalar_type,
        )?;
        charge_backing_roots(
            function,
            builder,
            row,
            source,
            length,
            |extent| SelectedMemoryAccessRole::ReadByteSpan {
                length: extent,
                obligation,
                accepted_fact,
            },
            unsliced_payload_offset(function, source),
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
            builder
                .constraints
                .keys
                .load8_indexed
                .ok_or_else(|| invalid())?,
            &[pointer, physical_index, output],
            operation_provenance(row, vec![index, length, definition.value]),
        )?;
        Ok(output)
    }

    pub(crate) fn charge_backing_roots(
        function: &crate::native_geometry::NativeFunction<'_>,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        place: PlaceId,
        length: ValueId,
        span: impl Fn(ValueId) -> SelectedMemoryAccessRole,
        sequence_offset: u32,
        sequence_count: u32,
        sequence: SelectedMemoryAccessRole,
    ) -> Result<(), SelectedInstructionError> {
        match crate::selection::byte_view_homes::view_backing_roots(
            function,
            &builder.transport.views,
            place,
            length,
        )? {
            Some(roots) => {
                for (root, extent) in roots {
                    memory(builder, row, root, 0, 0, span(extent))?;
                }
            }
            None => memory(
                builder,
                row,
                place,
                sequence_offset,
                sequence_count,
                sequence,
            )?,
        }
        Ok(())
    }

    pub(crate) fn unsliced_payload_offset(
        function: &crate::native_geometry::NativeFunction<'_>,
        place: PlaceId,
    ) -> u32 {
        if crate::selection::byte_concatenation_input::owner_layout_type(function, place).is_some()
        {
            8
        } else {
            0
        }
    }

    fn backing_index(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        place: PlaceId,
        index_register: VirtualRegisterId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let Some(view) = builder.transport.view(place).copied() else {
            return Ok(index_register);
        };
        let (index, length, obligation, accepted_fact) = match row.kind {
            LegalizedScalarInstructionKind::ByteSequenceRead {
                index,
                length,
                obligation,
                accepted_fact,
                ..
            }
            | LegalizedScalarInstructionKind::ByteSequenceWrite {
                index,
                length,
                obligation,
                accepted_fact,
                ..
            } => (index, length, obligation, accepted_fact),
            _ => return Err(invalid()),
        };
        let offset = transport_register(builder, place, 0)?;
        builder.emit(
            SelectedInstructionKind::ExactAddI64 {
                obligation,
                accepted_fact,
            },
            builder.constraints.keys.add_i64,
            &[view.byte_offset, index_register, offset],
            obligation_provenance(row, &[index, length, view.root_length], &[obligation]),
        )?;
        Ok(offset)
    }

    pub(crate) fn backing_pointer(
        function: &crate::native_geometry::NativeFunction<'_>,
        builder: &mut Builder<'_>,
        row: &impl crate::selection::construction::scalar_graph::call_arguments::InvocationArgumentSource,
        source: PlaceId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        if let Some(view) = builder.transport.view(source) {
            return Ok(view.backing_pointer);
        }
        crate::selection::construction::scalar_graph::structural::descriptor_operands::backing(
            function,
            builder,
            row,
            &terminal_psi::StructuralArgument {
                place: source,
                path: Vec::new(),
                access: terminal_psi::StructuralAccess::SharedBorrow,
            },
            &[],
        )
    }

    fn byte_sequence_length(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        source: PlaceId,
        length_byte_offset: u32,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let result = row.result.ok_or_else(|| invalid())?;
        if !matches!(length_byte_offset, 0 | 8)
            || result.scalar_type
                != ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?,
                )
        {
            return Err(invalid());
        }
        if let Some(view) = builder.transport.view(source) {
            let input = view.byte_length;
            let output =
                builder.register(result.value, result.definition_site, result.scalar_type)?;
            builder.emit_copy_i64(input, output, operation_provenance(row, vec![result.value]))?;
            return Ok(output);
        }
        let input = builder
            .transport
            .pointers
            .iter()
            .find(|(place, _)| *place == source)
            .map(|(_, register)| *register)
            .ok_or_else(|| invalid())?;
        let output = builder.register(result.value, result.definition_site, result.scalar_type)?;
        memory(
            builder,
            row,
            source,
            length_byte_offset,
            8,
            SelectedMemoryAccessRole::ReadPlace,
        )?;
        let provenance = operation_provenance(row, vec![result.value]);
        builder.emit_load64(length_byte_offset, input, output, provenance)?;
        Ok(output)
    }

    pub(crate) fn byte_predicate(
        function: &crate::native_geometry::NativeFunction<'_>,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let LegalizedScalarInstructionKind::ByteSequencePredicate { operand, predicate } =
            &row.kind
        else {
            return Err(invalid());
        };
        let result = row.result.ok_or_else(|| invalid())?;
        let integer = ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?,
        );
        let (_, count, _, count_type) =
            builder.resolve(operand.length()).ok_or_else(|| invalid())?;
        if result.scalar_type != ScalarType::Boolean || count_type != integer {
            return Err(invalid());
        }
        let base = byte_concatenation::operand_pointer(function, builder, row, operand)?;
        measured_span_reads(function, builder, row, operand)?;
        let cursor = byte_field_store::scratch(builder, 2)?;
        let byte = byte_field_store::scratch(builder, 3)?;
        let output = builder.register(result.value, result.definition_site, result.scalar_type)?;
        builder.emit(
            SelectedInstructionKind::ScanByteSequence {
                predicate: *predicate,
            },
            builder
                .constraints
                .keys
                .scan_byte_sequence
                .ok_or_else(|| invalid())?,
            &[base, count, cursor, byte, output],
            operation_provenance(row, vec![operand.length(), result.value]),
        )?;
        Ok(output)
    }

    pub(crate) fn byte_equal(
        function: &crate::native_geometry::NativeFunction<'_>,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let LegalizedScalarInstructionKind::ByteSequenceEqual { left, right } = &row.kind else {
            return Err(invalid());
        };
        let result = row.result.ok_or_else(|| invalid())?;
        let integer = ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?,
        );
        let (_, left_count, _, left_count_type) =
            builder.resolve(left.length()).ok_or_else(|| invalid())?;
        let (_, right_count, _, right_count_type) =
            builder.resolve(right.length()).ok_or_else(|| invalid())?;
        if result.scalar_type != ScalarType::Boolean
            || left_count_type != integer
            || right_count_type != integer
        {
            return Err(invalid());
        }
        let left_base = byte_concatenation::operand_pointer(function, builder, row, left)?;
        let right_base = byte_concatenation::operand_pointer(function, builder, row, right)?;
        measured_span_reads(function, builder, row, left)?;
        measured_span_reads(function, builder, row, right)?;
        let cursor = byte_field_store::scratch(builder, 4)?;
        let left_byte = byte_field_store::scratch(builder, 5)?;
        let right_byte = byte_field_store::scratch(builder, 6)?;
        let output = builder.register(result.value, result.definition_site, result.scalar_type)?;
        builder.emit(
            SelectedInstructionKind::CompareByteSequences,
            builder
                .constraints
                .keys
                .compare_byte_sequences
                .ok_or_else(|| invalid())?,
            &[
                left_base,
                left_count,
                right_base,
                right_count,
                cursor,
                left_byte,
                right_byte,
                output,
            ],
            operation_provenance(row, vec![left.length(), right.length(), result.value]),
        )?;
        Ok(output)
    }

    fn measured_span_reads(
        function: &crate::native_geometry::NativeFunction<'_>,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        operand: &ByteSequenceOperand,
    ) -> Result<(), SelectedInstructionError> {
        let (offset, roots) = crate::selection::construction::scalar_graph::structural::byte_concatenation::operand_span(function, builder, operand)?;
        for (root, length) in roots {
            memory(
                builder,
                row,
                root,
                offset,
                0,
                SelectedMemoryAccessRole::ReadMeasuredSpan { length },
            )?;
        }
        Ok(())
    }
}

pub(crate) mod element_establishment {
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::invalid;
    use crate::selection::construction::scalar_graph::structural::local_storage::{address, store};
    use crate::selection::construction::scalar_graph::structural::obligation_provenance;
    use crate::selection::construction::scalar_graph::structural::provenance;
    use crate::selection::construction::scalar_graph::structural::runtime_address::{
        cross_references, footprint, projection_origin, scale,
    };
    use crate::selection::construction::scalar_graph::structural::transport_register;
    use crate::selection::construction::scalar_graph::structural::{
        Builder, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
        SelectedInstructionKind, VirtualRegisterId,
    };
    use crate::selection::primitive_local_input::read_parameter;
    use crate::structural_inputs::structural_reference_input::collection_view_region_geometry;
    use selected_instructions::{LocalStorageSlotId, SelectedLocalStorageSlot};
    use semantic_vocabulary::{IntegerValue, StructuralPlaceKind};
    use terminal_psi::{StructuralAccess, StructuralTypeShape};

    pub(crate) fn establish(
        function: &crate::native_geometry::NativeFunction<'_>,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
    ) -> Result<(), SelectedInstructionError> {
        let LegalizedScalarInstructionKind::EstablishElementView {
            result,
            destination,
            source,
            element,
            region,
            accepted_fact,
            indices,
        } = &row.kind
        else {
            return Err(invalid());
        };
        if row.result.is_some()
            || result.place != *destination
            || !matches!(
                result.multiplicity,
                terminal_psi::StructuralMultiplicity::Unrestricted
                    | terminal_psi::StructuralMultiplicity::Affine
            )
            || !result.qualifications.is_empty()
            || result.projected_qualifications.iter().any(|row| {
                !matches!(
                    row.path.first(),
                    Some(terminal_psi::StructuralPathSegment::AllElements)
                )
            })
            || !result.claims.is_empty()
            || !matches!(source.access, terminal_psi::reading_borrow_pair_pattern!())
            || builder
                .transport
                .pointers
                .iter()
                .any(|(place, _)| *place == *destination)
        {
            return Err(invalid());
        }
        let Some(signature) = function.structural.as_ref() else {
            return Err(invalid());
        };
        let Some(place) = signature.structural_place(*destination) else {
            return Err(invalid());
        };
        if !matches!(
            place.kind,
            StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            } if producer == row.operation && structural_type == result.structural_type
        ) {
            return Err(invalid());
        }
        let Some(declaration) = signature.structural_type(result.structural_type) else {
            return Err(invalid());
        };
        match &declaration.shape {
            StructuralTypeShape::ElementView {
                element: view_element,
                ..
            } if *view_element == *element => {}
            StructuralTypeShape::ByteSequence(_)
                if declaration.shape.is_borrowed_byte_carrier()
                    && signature
                        .structural_type(*element)
                        .is_some_and(|element_declaration| {
                            matches!(
                                element_declaration.shape,
                                StructuralTypeShape::PrimitiveScalar(
                                    semantic_vocabulary::ScalarType::Integer(integer)
                                ) if Ok(integer)
                                    == semantic_vocabulary::IntegerType::new(
                                        semantic_vocabulary::IntegerSign::Unsigned,
                                        8,
                                    )
                            )
                        }) => {}
            _ => return Err(invalid()),
        }
        let parameter = signature
            .parameters
            .iter()
            .find(|parameter| parameter.semantic.place == source.place)
            .map(|parameter| &parameter.semantic)
            .or_else(|| {
                crate::selection::address_join_input::join(function, source.place)
                    .map(|(_, parameter)| parameter)
            });
        let home = crate::selection::aggregate_result_input::home(function, source.place);
        let local_root = home.map(|(_, home)| local_home_root(home));
        let block_home =
            crate::selection::aggregate_result_input::block_home(function, source.place);
        let descriptor_root = read_parameter(function, source.place).filter(|parameter| {
            terminal_semantics::view_descriptor_parameter(
                parameter,
                signature.structural_types.iter(),
            )
        });
        let Some(root) = parameter
            .or(local_root.as_ref())
            .or(block_home.map(|(_, declaration)| declaration))
            .or(descriptor_root)
        else {
            return Err(invalid());
        };
        let Some(geometry) = collection_view_region_geometry(
            root,
            source,
            result.structural_type,
            function.geometry(),
            *region,
        ) else {
            return Err(invalid());
        };
        if !crate::structural_inputs::runtime_indices_match(indices, &geometry.elements) {
            return Err(invalid());
        }
        let field_offset = geometry.offset;
        let element_count = geometry.extent;
        let descriptor = terminal_semantics::collection_view_descriptor(
            &declaration.shape,
            Some(element_count),
            *region,
            &source.path,
            signature.structural_type(*element),
        )
        .ok_or_else(invalid)?;
        let descriptor_size = match descriptor {
            terminal_semantics::CollectionViewDescriptor::StoredLength => 16,
            terminal_semantics::CollectionViewDescriptor::DeclaredLength(_) => 8,
        };
        let completed = if element_count == terminal_semantics::CollectionExtent::Fixed(0)
            && matches!(
                region,
                terminal_psi::ElementViewRegion::WholeCollection
                    | terminal_psi::ElementViewRegion::WholeRepresentation { .. }
            ) {
            None
        } else {
            crate::selection::construction::scalar_graph::structural::descriptor_operands::completed_view_words(function, builder, row, source, indices)?
        };
        if completed.is_some()
            && (geometry.storage != terminal_semantics::CollectionStorage::Borrowed
                || field_offset != 0
                || !indices.is_empty())
        {
            return Err(invalid());
        }
        let base = if completed.is_some() {
            None
        } else {
            let mut pointer_slots = geometry.pointer_slots.as_slice();
            let base = if element_count == terminal_semantics::CollectionExtent::Fixed(0)
                && matches!(
                    region,
                    terminal_psi::ElementViewRegion::WholeCollection
                        | terminal_psi::ElementViewRegion::WholeRepresentation { .. }
                ) {
                constant(builder, row, 0, 0)?
            } else if !geometry.pointer_slots.is_empty() {
                let (base, slots) = projection_origin(
                    function,
                    builder,
                    row,
                    source.place,
                    field_offset,
                    pointer_slots,
                )?;
                pointer_slots = slots;
                base.ok_or_else(invalid)?
            } else if parameter.is_some() || descriptor_root.is_some() {
                if source.path.is_empty() {
                    builder
                        .transport
                        .pointers
                        .iter()
                        .find(|(place, _)| *place == source.place)
                        .map(|(_, pointer)| *pointer)
                        .ok_or_else(|| invalid())?
                } else {
                    crate::selection::construction::scalar_graph::structural::runtime_address::projection_base(
                        function,
                        builder,
                        row,
                        source.place,
                        field_offset,
                    )?
                    .ok_or_else(|| invalid())?
                }
            } else {
                let mut homes = builder.transport.local_slots.iter().filter(|slot| {
                    Some(slot.id)
                        == home
                            .map(|(operation, _)| LocalStorageSlotId::Structural {
                                operation,
                                place: source.place,
                            })
                            .or_else(|| {
                                block_home.map(|(block, _)| {
                                    LocalStorageSlotId::StructuralBlockParameter {
                                        block,
                                        place: source.place,
                                    }
                                })
                            })
                });
                let (Some(home), None) = (homes.next().cloned(), homes.next()) else {
                    return Err(invalid());
                };
                address(builder, row, home.id, 0, home.byte_size, false)?
            };
            let base = if element_count == terminal_semantics::CollectionExtent::Fixed(0)
                && matches!(
                    region,
                    terminal_psi::ElementViewRegion::WholeCollection
                        | terminal_psi::ElementViewRegion::WholeRepresentation { .. }
                ) {
                base
            } else {
                let (base, leaf_index) =
                    cross_references(builder, row, source.place, base, indices, pointer_slots)?;
                scale(
                    builder,
                    row,
                    source.place,
                    field_offset,
                    base,
                    &indices[leaf_index..],
                )?
            };
            Some(base)
        };
        let slot = LocalStorageSlotId::Structural {
            operation: row.operation,
            place: *destination,
        };
        builder
            .transport
            .local_slots
            .push(SelectedLocalStorageSlot {
                id: slot,
                byte_size: descriptor_size,
                alignment: 8,
            });
        let backing = if let Some([backing, _]) = completed {
            backing
        } else {
            let base = base.ok_or_else(invalid)?;
            if element_count == terminal_semantics::CollectionExtent::Fixed(0)
                && matches!(
                    region,
                    terminal_psi::ElementViewRegion::WholeCollection
                        | terminal_psi::ElementViewRegion::WholeRepresentation { .. }
                )
            {
                base
            } else if geometry.storage == terminal_semantics::CollectionStorage::Borrowed {
                let pointer = transport_register(builder, *destination, 0)?;
                footprint(
                    builder,
                    row,
                    source.place,
                    field_offset,
                    8,
                    indices,
                    crate::selection::construction::scalar_graph::structural::runtime_address::Written::None,
                )?;
                builder.emit_load64(field_offset, base, pointer, provenance(row))?;
                pointer
            } else if field_offset == 0 {
                base
            } else {
                let offset = constant(builder, row, 0, u64::from(field_offset))?;
                let backing = transport_register(builder, *destination, 0)?;
                builder.emit_byte_view_address(base, offset, backing, provenance(row))?;
                backing
            }
        };
        let (backing, length) = match region {
            terminal_psi::ElementViewRegion::WholeCollection => {
                if accepted_fact.is_some() {
                    return Err(invalid());
                }
                let length = match element_count {
                    terminal_semantics::CollectionExtent::Fixed(length) => {
                        constant(builder, row, 8, length)?
                    }
                    terminal_semantics::CollectionExtent::LiveLength { .. } => {
                        let base = base.ok_or_else(invalid)?;
                        let header = field_offset.checked_sub(8).ok_or(invalid())?;
                        let pointer = transport_register(builder, *destination, 0)?;
                        footprint(
                            builder,
                            row,
                            source.place,
                            header,
                            8,
                            indices,
                            crate::selection::construction::scalar_graph::structural::runtime_address::Written::None,
                        )?;
                        builder.emit_load64(header, base, pointer, provenance(row))?;
                        pointer
                    }
                    terminal_semantics::CollectionExtent::StoredLength => {
                        if let Some([_, length]) = completed {
                            length
                        } else {
                            let base = base.ok_or_else(invalid)?;
                            let count_offset = field_offset.checked_add(8).ok_or(invalid())?;
                            let count = transport_register(builder, *destination, 8)?;
                            footprint(
                                builder,
                                row,
                                source.place,
                                count_offset,
                                8,
                                indices,
                                crate::selection::construction::scalar_graph::structural::runtime_address::Written::None,
                            )?;
                            builder.emit_load64(count_offset, base, count, provenance(row))?;
                            count
                        }
                    }
                };
                (backing, length)
            }
            terminal_psi::ElementViewRegion::WholeRepresentation {
                source_length,
                obligation,
            } => {
                if accepted_fact.is_none() || geometry.length_scale == 0 {
                    return Err(invalid());
                }
                let (_, count, _, count_type) =
                    builder.resolve(*source_length).ok_or_else(invalid)?;
                if !matches!(count_type,
                    semantic_vocabulary::ScalarType::Integer(integer)
                        if Ok(integer) == semantic_vocabulary::IntegerType::new(
                            semantic_vocabulary::IntegerSign::Unsigned, 64))
                {
                    return Err(invalid());
                }
                let length = if geometry.length_scale == 1 {
                    count
                } else {
                    let factor = constant(builder, row, 8, geometry.length_scale)?;
                    let scaled = transport_register(builder, *destination, 8)?;
                    builder.emit(
                        SelectedInstructionKind::WrappingMultiplyI64,
                        builder.constraints.keys.multiply_i64,
                        &[count, factor, scaled],
                        obligation_provenance(row, &[*source_length], &[*obligation]),
                    )?;
                    scaled
                };
                (backing, length)
            }
            terminal_psi::ElementViewRegion::RepresentationBytes {
                start,
                end,
                obligation,
            } => {
                let fact = accepted_fact.ok_or_else(|| invalid())?;
                let (_, start_register, _, start_type) =
                    builder.resolve(*start).ok_or_else(|| invalid())?;
                let (_, end_register, _, end_type) =
                    builder.resolve(*end).ok_or_else(|| invalid())?;
                let word = semantic_vocabulary::ScalarType::Integer(
                    semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        64,
                    )
                    .map_err(|_| invalid())?,
                );
                if start_type != word || end_type != word || geometry.stride == 0 {
                    return Err(invalid());
                }
                let region_provenance = obligation_provenance(row, &[*start, *end], &[*obligation]);
                let selected_backing = transport_register(builder, *destination, 0)?;
                builder.emit_byte_view_address(
                    backing,
                    start_register,
                    selected_backing,
                    region_provenance.clone(),
                )?;
                let bytes = transport_register(builder, *destination, 8)?;
                builder.emit(
                    SelectedInstructionKind::ExactSubtractI64 {
                        obligation: *obligation,
                        accepted_fact: fact,
                    },
                    builder.constraints.keys.subtract_i64,
                    &[end_register, start_register, bytes],
                    region_provenance.clone(),
                )?;
                let stride = constant(builder, row, 8, u64::from(geometry.stride))?;
                let length = transport_register(builder, *destination, 8)?;
                let mut operands = vec![bytes, stride, length];
                if builder.environment.target().architecture == target::Architecture::X86_64 {
                    operands.push(crate::selection::construction::scalar_graph::integer_arithmetic::division_scratch(builder)?);
                }
                builder.emit(
                    SelectedInstructionKind::ExactDivideU64 {
                        obligation: *obligation,
                        accepted_fact: fact,
                    },
                    builder.constraints.keys.divide_u64,
                    &operands,
                    region_provenance.clone(),
                )?;
                (selected_backing, length)
            }
        };
        store(builder, row, slot, 0, backing)?;
        if descriptor == terminal_semantics::CollectionViewDescriptor::StoredLength {
            store(builder, row, slot, 8, length)?;
        }
        let descriptor = address(builder, row, slot, 0, descriptor_size, true)?;
        builder.transport.pointers.push((*destination, descriptor));
        Ok(())
    }

    fn constant(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        offset: u32,
        value: u64,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let LegalizedScalarInstructionKind::EstablishElementView { destination, .. } = &row.kind
        else {
            return Err(invalid());
        };
        let register = transport_register(builder, *destination, offset)?;
        builder.emit_materialize_i64(
            IntegerValue::Unsigned(u128::from(value)),
            register,
            provenance(row),
        )?;
        Ok(register)
    }

    fn local_home_root(
        home: &terminal_psi::StructuralOperationResult,
    ) -> terminal_psi::StructuralParameterDeclaration {
        terminal_psi::StructuralParameterDeclaration {
            place: home.place,
            position: 0,
            structural_type: home.structural_type,
            multiplicity: home.multiplicity,
            access: StructuralAccess::Owned,
            qualifications: home.qualifications.clone(),
            projected_qualifications: home.projected_qualifications.clone(),
            additional_input_qualifications: home.additional_input_qualifications.clone(),
        }
    }
}

pub(crate) mod element_subslice {
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::invalid;
    use crate::selection::construction::scalar_graph::structural::transport_register;
    use crate::selection::construction::scalar_graph::structural::{
        Builder, IntegerSign, IntegerType, LegalizedScalarInstruction,
        LegalizedScalarInstructionKind, ScalarType, SelectedInstructionKind,
        SelectedInstructionProvenance, obligation_provenance,
    };
    use crate::selection::element_view_homes::ElementViewHomes;
    use semantic_vocabulary::IntegerValue;

    pub(crate) fn create(
        function: &crate::native_geometry::NativeFunction<'_>,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
    ) -> Result<(), SelectedInstructionError> {
        let LegalizedScalarInstructionKind::ElementViewSubslice {
            result,
            source,
            indices,
            start,
            end,
            length,
            obligation,
            accepted_fact,
        } = &row.kind
        else {
            return Err(invalid());
        };
        let signature = function.structural.as_ref().ok_or_else(|| invalid())?;
        let (geometry, _) =
            crate::selection::construction::scalar_graph::structural::descriptor_operands::source_geometry(function, source).ok_or_else(|| invalid())?;
        let source_shape = signature
            .structural_types
            .iter()
            .find(|row| row.id == geometry.selected);
        let result_shape = signature
            .structural_types
            .iter()
            .find(|row| row.id == result.structural_type);
        let same_element = source_shape.zip(result_shape).is_some_and(|(source, result)|
            matches!((&source.shape, &result.shape),
                (terminal_psi::StructuralTypeShape::ElementView { element: source, .. },
                 terminal_psi::StructuralTypeShape::ElementView { element: result, .. }) if source == result));
        if crate::selection::established_view_input::view_access(function, result.place).is_none() {
            return Err(SelectedInstructionError::custody_because(
                "element subslice does not retain readable source descriptor access",
            ));
        }
        if row.result.is_some()
            || !same_element
            || !result.qualifications.is_empty()
            || result.projected_qualifications.iter().any(|row| {
                !matches!(
                    row.path.first(),
                    Some(terminal_psi::StructuralPathSegment::AllElements)
                )
            })
            || !result.claims.is_empty()
            || builder
                .transport
                .pointers
                .iter()
                .any(|(place, _)| *place == result.place)
            || builder.transport.element_view(result.place).is_some()
        {
            return Err(invalid());
        }
        let integer = ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?,
        );
        let (_, start_register, _, start_type) =
            builder.resolve(*start).ok_or_else(|| invalid())?;
        let (_, end_register, _, end_type) = builder.resolve(*end).ok_or_else(|| invalid())?;
        let (_, _, _, length_type) = builder.resolve(*length).ok_or_else(|| invalid())?;
        if start_type != integer || end_type != integer || length_type != integer {
            return Err(invalid());
        }
        let stride =
            crate::selection::construction::scalar_graph::structural::descriptor_operands::element_stride(function, source).ok_or_else(|| invalid())?;
        let parent = source
            .path
            .is_empty()
            .then(|| builder.transport.element_view(source.place).copied())
            .flatten();
        let backing = match parent {
            Some(parent) => parent.backing_pointer,
            None => crate::selection::construction::scalar_graph::structural::descriptor_operands::backing(function, builder, row, source, indices)?,
        };
        let stride_register = transport_register(builder, result.place, 0)?;
        let scaled_start = transport_register(builder, result.place, 0)?;
        builder.emit_materialize_i64(
            IntegerValue::Unsigned(u128::from(stride)),
            stride_register,
            obligation_provenance(row, &[*start, *end, *length], &[*obligation]),
        )?;
        builder.emit(
            SelectedInstructionKind::WrappingMultiplyI64,
            builder.constraints.keys.multiply_i64,
            &[start_register, stride_register, scaled_start],
            obligation_provenance(row, &[*start, *end, *length], &[*obligation]),
        )?;
        let (byte_offset, root_length) = if let Some(parent) = parent {
            let byte_offset = transport_register(builder, result.place, 0)?;
            builder.emit(
                SelectedInstructionKind::ExactAddI64 {
                    obligation: *obligation,
                    accepted_fact: *accepted_fact,
                },
                builder.constraints.keys.add_i64,
                &[parent.byte_offset, scaled_start, byte_offset],
                obligation_provenance(
                    row,
                    &[*start, *end, *length, parent.root_length],
                    &[*obligation],
                ),
            )?;
            (byte_offset, parent.root_length)
        } else {
            (scaled_start, *length)
        };
        let element_length = transport_register(builder, result.place, 8)?;
        builder.emit(
            SelectedInstructionKind::ExactSubtractI64 {
                obligation: *obligation,
                accepted_fact: *accepted_fact,
            },
            builder.constraints.keys.subtract_i64,
            &[end_register, start_register, element_length],
            SelectedInstructionProvenance {
                operations: vec![row.operation],
                values: vec![*start, *end, *length],
                obligations: vec![*obligation],
                work: row.work.clone(),
                ..Default::default()
            },
        )?;
        builder.transport.element_views.push(ElementViewHomes {
            place: result.place,
            backing_pointer: backing,
            byte_offset,
            element_length,
            root_length,
            element_stride: stride,
        });
        if crate::selection::established_view_input::requires_descriptor(function, result.place) {
            let slot =
                builder
                    .transport
                    .push_structural_local_slot(row.operation, result.place, 16, 8);
            let pointer = transport_register(builder, result.place, 0)?;
            builder.emit_byte_view_address(
                backing,
                byte_offset,
                pointer,
                obligation_provenance(row, &[*start, *end, *length, root_length], &[*obligation]),
            )?;
            crate::selection::construction::scalar_graph::structural::local_storage::store(
                builder, row, slot, 0, pointer,
            )?;
            crate::selection::construction::scalar_graph::structural::local_storage::store(
                builder,
                row,
                slot,
                8,
                element_length,
            )?;
            let descriptor =
                crate::selection::construction::scalar_graph::structural::local_storage::address(
                    builder, row, slot, 0, 16, false,
                )?;
            builder.transport.pointers.push((result.place, descriptor));
        }
        Ok(())
    }
}

pub(crate) mod element_view_write {
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::invalid;
    use crate::selection::construction::scalar_graph::structural::obligation_provenance;
    use crate::selection::construction::scalar_graph::structural::operation_provenance;
    use crate::selection::construction::scalar_graph::structural::scaled_element_index;
    use crate::selection::construction::scalar_graph::structural::transport_register;
    use crate::selection::construction::scalar_graph::structural::{
        Builder, IntegerSign, IntegerType, LegalizedScalarInstruction,
        LegalizedScalarInstructionKind, ScalarType, SelectedMemoryAccessRole, memory,
    };
    use terminal_psi::StructuralTypeShape;

    pub(crate) fn write(
        function: &crate::native_geometry::NativeFunction<'_>,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
    ) -> Result<(), SelectedInstructionError> {
        let LegalizedScalarInstructionKind::ElementViewWrite {
            ref path,
            ref destination,
            ref indices,
            index,
            value,
            length,
            obligation,
            accepted_fact,
        } = row.kind
        else {
            return Err(invalid());
        };
        let (_, index_register, _, index_type) = builder.resolve(index).ok_or_else(|| invalid())?;
        let (_, value_register, _, value_type) = builder.resolve(value).ok_or_else(|| invalid())?;
        if row.result.is_some()
            || index_type
                != ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?,
                )
            || crate::structural_inputs::structural_reference_input::scalar_shape(value_type)
                .is_none()
        {
            return Err(invalid());
        }
        let stride = crate::selection::construction::scalar_graph::structural::descriptor_operands::element_stride(function, destination)
            .ok_or_else(|| invalid())?;
        let Some(_) = function.structural.as_ref() else {
            return Err(invalid());
        };
        let Some((_, StructuralTypeShape::ElementView { element, .. })) =
            crate::selection::construction::scalar_graph::structural::descriptor_operands::source_geometry(function, destination)
        else {
            return Err(invalid());
        };
        let (byte_offset, byte_count) =
            crate::structural_inputs::structural_reference_input::element_leaf_geometry(
                element,
                path,
                value_type,
                function.geometry(),
            )
            .ok_or_else(|| invalid())?;
        let pointer = if destination.path.is_empty() {
            crate::selection::construction::scalar_graph::structural::element_views::element_backing_pointer(builder, row, destination.place)?
        } else {
            crate::selection::construction::scalar_graph::structural::descriptor_operands::backing(
                function,
                builder,
                row,
                destination,
                indices,
            )?
        };
        let physical_index = scaled_element_index(
            builder,
            destination.place,
            stride,
            index_register,
            index,
            length,
            obligation,
            accepted_fact,
            row,
        )?;
        memory(
            builder,
            row,
            destination.place,
            byte_offset,
            byte_count,
            SelectedMemoryAccessRole::WriteElementView {
                index,
                length,
                obligation,
                accepted_fact,
            },
        )?;
        let address = transport_register(builder, destination.place, 0)?;
        builder.emit_byte_view_address(
            pointer,
            physical_index,
            address,
            obligation_provenance(row, &[index, length], &[obligation]),
        )?;
        builder.emit_store(
            byte_offset,
            u8::try_from(byte_count).map_err(|_| invalid())?,
            address,
            value_register,
            operation_provenance(row, vec![index, value, length]),
        )
    }
}

pub(crate) mod element_views {
    use crate::SelectedInstructionError;
    use crate::selection::construction::scalar_graph::structural::invalid;
    use crate::selection::construction::scalar_graph::structural::obligation_provenance;
    use crate::selection::construction::scalar_graph::structural::operation_provenance;
    use crate::selection::construction::scalar_graph::structural::provenance;
    use crate::selection::construction::scalar_graph::structural::scaled_element_index;
    use crate::selection::construction::scalar_graph::structural::transport_register;
    use crate::selection::construction::scalar_graph::structural::{
        Builder, IntegerSign, IntegerType, LegalizedScalarInstruction,
        LegalizedScalarInstructionKind, PlaceId, ScalarType, SelectedInstructionProvenance,
        SelectedMemoryAccessRole, VirtualRegisterId, memory,
    };
    use crate::selection::established_view_input::element_view_type;
    use crate::selection::scalar_call_abi::load_transfer;
    use crate::structural_inputs::structural_reference_input::{align, element_leaf_projection};
    use terminal_psi::StructuralTypeShape;

    pub(crate) fn element_stride(
        function: &crate::native_geometry::NativeFunction<'_>,
        builder: &Builder<'_>,
        source: PlaceId,
    ) -> Option<u32> {
        if let Some(view) = builder.transport.element_view(source) {
            return Some(view.element_stride);
        }
        let view_type = element_view_type(function, source)?;
        function.structural.as_ref()?;
        let declaration = terminal_psi::find_structural_type(function.geometry(), view_type)?;
        let StructuralTypeShape::ElementView { element, .. } = &declaration.shape else {
            return None;
        };
        let shape = crate::structural_inputs::shape(*element, function.geometry())?;
        align(shape.byte_size, shape.alignment).filter(|stride| *stride != 0)
    }

    pub(crate) fn element_observation(
        function: &crate::native_geometry::NativeFunction<'_>,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        match &row.kind {
            LegalizedScalarInstructionKind::ElementViewRead { source, .. } => {
                if element_view_type(function, *source).is_none() {
                    return Err(invalid());
                }
                element_view_read(function, builder, row)
            }
            LegalizedScalarInstructionKind::ElementViewLength {
                source,
                length_byte_offset,
                ..
            } => {
                if *length_byte_offset != 8 {
                    return Err(invalid());
                }
                if source.path.is_empty() && builder.transport.element_view(source.place).is_some()
                {
                    element_view_length(builder, row, source.place, *length_byte_offset)
                } else {
                    crate::selection::construction::scalar_graph::structural::primitive_locals::read(
                        function, builder, row,
                    )
                }
            }
            _ => Err(invalid()),
        }
    }

    fn element_view_read(
        function: &crate::native_geometry::NativeFunction<'_>,
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        let LegalizedScalarInstructionKind::ElementViewRead {
            source,
            index,
            length,
            obligation,
            accepted_fact,
            ref path,
            ref indices,
        } = row.kind
        else {
            return Err(invalid());
        };
        let definition = row.result.ok_or_else(|| invalid())?;
        let (_, index_register, _, index_type) = builder.resolve(index).ok_or_else(|| invalid())?;
        if index_type
            != ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?,
            )
        {
            return Err(invalid());
        }
        let stride = element_stride(function, builder, source).ok_or_else(|| invalid())?;
        let Some(_) = function.structural.as_ref() else {
            return Err(invalid());
        };
        let Some(view_type) = element_view_type(function, source) else {
            return Err(invalid());
        };
        let element = terminal_psi::find_structural_type(function.geometry(), view_type)
            .and_then(|declaration| match declaration.shape {
                StructuralTypeShape::ElementView { element, .. } => Some(element),
                _ => None,
            })
            .ok_or_else(|| invalid())?;
        let (byte_offset, byte_count, elements) =
            element_leaf_projection(element, path, definition.scalar_type, function.geometry())
                .ok_or_else(|| invalid())?;
        if !crate::structural_inputs::runtime_indices_match(indices, &elements) {
            return Err(invalid());
        }
        let pointer = element_backing_pointer(builder, row, source)?;
        let physical_index = scaled_element_index(
            builder,
            source,
            stride,
            index_register,
            index,
            length,
            obligation,
            accepted_fact,
            row,
        )?;
        let output = builder.register(
            definition.value,
            definition.definition_site,
            definition.scalar_type,
        )?;
        let address = transport_register(builder, source, 0)?;
        builder.emit_byte_view_address(
            pointer,
            physical_index,
            address,
            obligation_provenance(row, &[index, length], &[obligation]),
        )?;
        let address =
            crate::selection::construction::scalar_graph::structural::runtime_address::scale(
                builder,
                row,
                source,
                byte_offset,
                address,
                indices,
            )?;
        let (address, displacement) =
            crate::selection::construction::scalar_graph::structural::memory_pointer(
                builder,
                row,
                source,
                address,
                byte_offset,
                byte_count,
            )?;
        memory(
            builder,
            row,
            source,
            byte_offset,
            byte_count,
            SelectedMemoryAccessRole::ReadElementView {
                index,
                length,
                obligation,
                accepted_fact,
            },
        )?;
        if !indices.is_empty() {
            crate::selection::construction::scalar_graph::structural::runtime_address::footprint(
                builder,
                row,
                source,
                byte_offset,
                byte_count,
                indices,
                crate::selection::construction::scalar_graph::structural::runtime_address::Written::None,
            )?;
        }
        let (kind, constraint) = load_transfer(&builder.constraints.keys, byte_count, displacement)
            .ok_or_else(invalid)?;
        builder.emit(
            kind,
            constraint.ok_or_else(|| invalid())?,
            &[address, output],
            operation_provenance(row, vec![index, length, definition.value]),
        )?;
        crate::selection::construction::scalar_graph::structural::primitive_locals::normalize_signed_load(builder, definition, output)
    }

    fn element_view_length(
        builder: &mut Builder<'_>,
        row: &LegalizedScalarInstruction,
        source: PlaceId,
        length_byte_offset: u32,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        if length_byte_offset != 8 {
            return Err(invalid());
        }
        let definition = row.result.ok_or_else(|| invalid())?;
        if definition.scalar_type
            != ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?,
            )
        {
            return Err(invalid());
        }
        let output = builder.register(
            definition.value,
            definition.definition_site,
            definition.scalar_type,
        )?;
        let view = builder
            .transport
            .element_view(source)
            .copied()
            .ok_or_else(|| invalid())?;
        builder.emit_copy_i64(
            view.element_length,
            output,
            SelectedInstructionProvenance {
                operations: vec![row.operation],
                values: vec![definition.value, view.root_length],
                ..Default::default()
            },
        )?;
        Ok(output)
    }

    pub(crate) fn element_backing_pointer(
        builder: &mut Builder<'_>,
        row: &impl crate::selection::construction::scalar_graph::call_arguments::InvocationArgumentSource,
        source: PlaceId,
    ) -> Result<VirtualRegisterId, SelectedInstructionError> {
        if let Some(view) = builder.transport.element_view(source) {
            return Ok(view.backing_pointer);
        }
        let descriptor = builder
            .transport
            .pointers
            .iter()
            .find(|(place, _)| *place == source)
            .map(|(_, register)| *register)
            .ok_or_else(|| invalid())?;
        let pointer = transport_register(builder, source, 0)?;
        memory(
            builder,
            row,
            source,
            0,
            8,
            SelectedMemoryAccessRole::ReadPlace,
        )?;
        builder.emit_load64(0, descriptor, pointer, provenance(row))?;
        Ok(pointer)
    }
}
