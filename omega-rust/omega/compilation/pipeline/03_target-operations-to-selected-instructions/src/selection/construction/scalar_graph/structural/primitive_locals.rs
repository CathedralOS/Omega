//! Establish and access the original primitive referent in the activation frame.
use super::{
    Builder, IntegerSign, LegalizedScalarFunction, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind, ScalarType, SelectedInstructionProvenance,
    SelectedMemoryAccessRole, VirtualRegisterId, memory,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::invalid;
use crate::selection::construction::scalar_graph::structural::local_storage;
use crate::selection::scalar_call_abi::load_transfer;
use selected_instructions::LocalStorageSlotId;

pub(super) fn write(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let (place, value, establishing) = match &row.kind {
        LegalizedScalarInstructionKind::EstablishPrimitiveLocal { result, value, .. } => {
            (result.place, *value, true)
        }
        LegalizedScalarInstructionKind::PrimitiveLocalStore { destination, value } => {
            (*destination, *value, false)
        }
        _ => return Err(invalid()),
    };
    let (producer, _, scalar, shape) =
        crate::selection::primitive_local_input::local(source, place).ok_or_else(|| invalid())?;
    if row.result.is_some() || scalar != value.scalar_type {
        return Err(invalid());
    }
    let pointer = if establishing {
        if producer != row.operation
            || builder
                .transport
                .pointers
                .iter()
                .any(|(stored, _)| *stored == place)
        {
            return Err(invalid());
        }
        let slot = LocalStorageSlotId::Structural {
            operation: producer,
            place,
        };
        builder.transport.push_structural_local_slot(
            producer,
            place,
            shape.byte_size,
            shape.alignment,
        );
        let pointer = local_storage::address(builder, row, slot, 0, shape.byte_size, false)?;
        builder.transport.pointers.push((place, pointer));
        pointer
    } else {
        builder
            .transport
            .pointers
            .iter()
            .find(|(stored, _)| *stored == place)
            .map(|(_, pointer)| *pointer)
            .ok_or_else(|| invalid())?
    };
    let (_, value_register, _, actual) = builder.resolve(value.value).ok_or_else(|| invalid())?;
    if actual != scalar {
        return Err(invalid());
    }
    let byte_size = u8::try_from(shape.byte_size).map_err(|_| invalid())?;
    memory(
        builder,
        row,
        place,
        0,
        u32::from(byte_size),
        SelectedMemoryAccessRole::WritePlace,
    )?;
    builder.emit_store(
        0,
        byte_size,
        pointer,
        value_register,
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![value.value],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(())
}
pub(in crate::selection) fn read(
    source: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let (place, byte_offset, pointer_slots) =
        crate::selection::primitive_local_input::read_geometry(source, row)
            .ok_or_else(|| invalid())?;
    let definition = row.result.ok_or_else(|| invalid())?;
    let observes_descriptor_home = matches!(&row.kind,
        LegalizedScalarInstructionKind::ElementViewLength { source, .. }
        | LegalizedScalarInstructionKind::ByteSequenceLength { source, .. }
        if source.path.is_empty());
    let (pointer, pointer_slots) = if observes_descriptor_home {
        // A whole descriptor's metadata is in its own home.
        (
            Some(super::place_pointer(builder, row, place, 0)?),
            pointer_slots.as_slice(),
        )
    } else {
        super::runtime_address::projection_origin(
            source,
            builder,
            row,
            place,
            byte_offset,
            &pointer_slots,
        )?
    };
    let pointer = pointer.ok_or_else(invalid)?;
    let pointer = if matches!(&row.kind,
        LegalizedScalarInstructionKind::StructuralScalarFieldRead { source, .. }
        if matches!(source.path.first(), Some(terminal_psi::StructuralPathSegment::Referent)))
    {
        super::reference::load(builder, row, place, pointer)?
    } else {
        pointer
    };
    // A primitive leaf reached through runtime elements scales each selector
    // into the address first; every other read loads at its static offset.
    let indices: &[legalized_operations::LegalizedRuntimeIndexOperand] = match &row.kind {
        LegalizedScalarInstructionKind::PrimitiveScalarRead { indices, .. }
        | LegalizedScalarInstructionKind::StructuralByteSequenceFieldLength { indices, .. }
        | LegalizedScalarInstructionKind::ElementViewLength { indices, .. }
        | LegalizedScalarInstructionKind::ByteSequenceLength { indices, .. } => indices,
        _ => &[],
    };
    if let Some((address, codec)) = super::runtime_address::scalar_reference(
        source,
        builder,
        row,
        place,
        pointer,
        indices,
        pointer_slots,
    )? {
        if byte_offset != 0 {
            return Err(invalid());
        }
        return super::encoded_scalar::read(builder, row, place, address, codec);
    }
    let (pointer, leaf_index) = super::runtime_address::cross_references(
        builder,
        row,
        place,
        pointer,
        indices,
        pointer_slots,
    )?;
    let indices = &indices[leaf_index..];
    let pointer =
        super::runtime_address::scale(builder, row, place, byte_offset, pointer, indices)?;
    if let Some(fragments) =
        crate::selection::primitive_local_input::read_storage_fragments(source, row)
    {
        return super::fragmented_scalar::read(
            builder,
            row,
            place,
            pointer,
            byte_offset,
            fragments,
            indices,
        );
    }
    let codec = crate::selection::primitive_local_input::read_storage_codec(source, row)
        .ok_or_else(|| invalid())?;
    let canonical = runtime_abi::ScalarStorageCodec::from_scalar(definition.scalar_type)
        .ok_or_else(|| invalid())?;
    if codec != canonical || matches!(codec.byte_size(), 3 | 5 | 6 | 7) {
        let address =
            super::encoded_scalar::backing_address(builder, row, place, pointer, byte_offset)?;
        let codec = super::encoded_scalar::constant_codec(builder, row, place, codec)?;
        return super::encoded_scalar::read(builder, row, place, address, codec);
    }
    let shape = crate::selection::scalar_call_abi::scalar_shape(definition.scalar_type)
        .ok_or_else(|| invalid())?;
    let (pointer, displacement) =
        super::memory_pointer(builder, row, place, pointer, byte_offset, shape.byte_size)?;
    let output = builder.register(
        definition.value,
        definition.definition_site,
        definition.scalar_type,
    )?;
    let (instruction, constraint) =
        load_transfer(&builder.constraints.keys, shape.byte_size, displacement)
            .ok_or_else(invalid)?;
    super::runtime_address::footprint(
        builder,
        row,
        place,
        byte_offset,
        shape.byte_size,
        indices,
        super::runtime_address::Written::None,
    )?;
    builder.emit(
        instruction,
        constraint.ok_or_else(|| invalid())?,
        &[pointer, output],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: indices
                .iter()
                .map(|index| index.operand.value)
                .chain([definition.value])
                .collect(),
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    normalize_signed_load(builder, definition, output)
}

/// The exact-width load zero-extends raw bits. Restore a narrow signed value
/// before any consumer, not only at a subsequent ABI boundary. Only the
/// memory load carries the authored read's operation and fuel.
pub(super) fn normalize_signed_load(
    builder: &mut Builder<'_>,
    definition: legalized_operations::LegalizedValueDefinition,
    output: VirtualRegisterId,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    if !matches!(definition.scalar_type, ScalarType::Integer(integer)
        if integer.sign() == IntegerSign::Signed && matches!(integer.bits(), 8 | 16 | 32))
    {
        return Ok(output);
    }
    let normalized = builder.register(
        definition.value,
        definition.definition_site,
        definition.scalar_type,
    )?;
    builder.emit(
        crate::selection::scalar_call_abi::integer_carrier_normalization(definition.scalar_type),
        builder.constraints.keys.copy_i64,
        &[output, normalized],
        SelectedInstructionProvenance {
            values: vec![definition.value],
            ..Default::default()
        },
    )?;
    Ok(normalized)
}

/// Expose the original referent pointer. Reference descriptors carry both an
/// address and an encoding; only the address is observed, never the codec or
/// the descriptor's own frame home. Offset application matches ordinary reads.
pub(super) fn address(
    source: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let (place, byte_offset, slots) =
        crate::selection::primitive_local_input::read_geometry(source, row).ok_or_else(|| {
            SelectedInstructionError::custody_because(
                "storage address lost its checked referent geometry",
            )
        })?;
    let LegalizedScalarInstructionKind::StorageAddress { indices, .. } = &row.kind else {
        return Err(invalid());
    };
    let (pointer, slots) = super::runtime_address::projection_origin(
        source,
        builder,
        row,
        place,
        byte_offset,
        &slots,
    )?;
    let pointer = match pointer {
        Some(pointer) => pointer,
        // The common resolver authenticates a completed local home. It cannot
        // create storage for missing ABI fragments or an unavailable loan.
        None => super::place_pointer(builder, row, place, 0)?,
    };
    let address = if let Some((address, _)) = super::runtime_address::scalar_reference(
        source, builder, row, place, pointer, indices, slots,
    )? {
        if byte_offset != 0 {
            return Err(invalid());
        }
        address
    } else {
        let (pointer, leaf) =
            super::runtime_address::cross_references(builder, row, place, pointer, indices, slots)?;
        let pointer = super::runtime_address::scale(
            builder,
            row,
            place,
            byte_offset,
            pointer,
            &indices[leaf..],
        )?;
        super::encoded_scalar::backing_address(builder, row, place, pointer, byte_offset)?
    };
    let result = row.result.ok_or_else(|| {
        SelectedInstructionError::custody_because("storage address lost its declared addr result")
    })?;
    let output = builder.register(result.value, result.definition_site, result.scalar_type)?;
    // ABI pointers use their transport carrier internally. The representation
    // query publishes those bits as its declared addr value, not a value cast.
    builder.emit_copy_i64(address, output, super::provenance(row))?;
    Ok(output)
}
