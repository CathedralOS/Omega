//! Exact writes through original borrowed pointers, owned result homes, or an
//! owned arrival's block home. A path through runtime-selected elements scales
//! each selector into the address first (`runtime_address`); the store then
//! applies the static offset.
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    SelectedInstructionProvenance,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::invalid;
use crate::structural_inputs::structural_reference_input::runtime_indices_match;

pub(super) fn emit(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let (destination, value, byte_offset, byte_size, indices, reference_destination, pointer_slots) =
        match &row.kind {
            LegalizedScalarInstructionKind::StructuralScalarFieldStore {
                destination,
                path,
                field,
                value,
                byte_offset,
                byte_size,
                indices,
            } => {
                let signature = source.structural.as_ref().ok_or_else(|| invalid())?;
                let reference = crate::selection::reference_input::store_destination(
                    source,
                    destination,
                    path,
                    terminal_psi::StructuralAccess::WriteOnlyBorrow,
                );
                let geometry =
                    crate::structural_inputs::structural_reference_input::field_store_geometry(
                        destination.structural_type(),
                        path,
                        *field,
                        value.scalar_type,
                        &signature.structural_types,
                    )
                    .ok_or_else(|| invalid())?;
                if (geometry.byte_offset, geometry.byte_size) != (*byte_offset, *byte_size)
                    || !runtime_indices_match(indices, &geometry.elements)
                    // View indexing owns its descriptor access separately; only
                    // an explicit reference crossing needs this carrier join.
                    || path.contains(&terminal_psi::StructuralPathSegment::Referent)
                        && reference.is_none()
                {
                    return Err(invalid());
                }
                (
                    destination.clone(),
                    value,
                    *byte_offset,
                    *byte_size,
                    indices,
                    reference.is_some(),
                    geometry.pointer_slots,
                )
            }
            LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
                destination,
                path,
                value,
                byte_offset,
                byte_size,
                indices,
            } => {
                let signature = source.structural.as_ref().ok_or_else(|| invalid())?;
                let geometry =
                    crate::structural_inputs::structural_reference_input::primitive_store(
                        destination,
                        path,
                        value.scalar_type,
                        &signature.structural_types,
                    )
                    .ok_or_else(|| invalid())?;
                // Custody belongs to exact places, not the function-wide
                // entry roster. A primitive write preserves unrelated claims;
                // the declared destination and projection checks retain
                // its storage authority and exact scalar geometry.
                if (geometry.byte_offset, geometry.byte_size)
                    != (*byte_offset, u32::from(*byte_size))
                    || !runtime_indices_match(indices, &geometry.elements)
                {
                    return Err(invalid());
                }
                (
                    destination.clone(),
                    value,
                    *byte_offset,
                    u32::from(*byte_size),
                    indices,
                    false,
                    geometry.pointer_slots,
                )
            }
            _ => return Err(invalid()),
        };
    let declared = match &destination {
        abstract_operations::StructuralScalarStoreDestination::Parameter(parameter) => {
            crate::selection::arrival_parameter_input::store_destination(source, parameter)
        }
        abstract_operations::StructuralScalarStoreDestination::OwnedResult { producer, result } => {
            reference_destination
                || crate::selection::aggregate_result_input::owned_store_destination(
                    source, *producer, result,
                )
        }
        // An owned arrival's leaf lands in its own block home, the slot
        // whose address `block_entry` placed in the transport pointers.
        abstract_operations::StructuralScalarStoreDestination::OwnedBlockParameter {
            block,
            parameter,
        } => crate::selection::owned_block_home_input::store_destination(source, *block, parameter),
    };
    if row.result.is_some() || !declared {
        return Err(invalid());
    }
    let (pointer, pointer_slots) = super::runtime_address::projection_origin(
        source,
        builder,
        row,
        destination.place(),
        byte_offset,
        &pointer_slots,
    )?;
    let pointer = pointer.ok_or_else(invalid)?;
    if let Some((address, codec)) = super::runtime_address::scalar_reference(
        source,
        builder,
        row,
        destination.place(),
        pointer,
        indices,
        pointer_slots,
    )? {
        if byte_offset != 0 {
            return Err(invalid());
        }
        return super::encoded_scalar::write(
            builder,
            row,
            destination.place(),
            address,
            codec,
            *value,
        );
    }
    let (pointer, leaf_index) = super::runtime_address::cross_references(
        builder,
        row,
        destination.place(),
        pointer,
        indices,
        pointer_slots,
    )?;
    let leaf_indices = &indices[leaf_index..];
    let (_, register, _, scalar) = builder.resolve(value.value).ok_or_else(|| invalid())?;
    if scalar != value.scalar_type {
        return Err(invalid());
    }
    let address = super::runtime_address::scale(
        builder,
        row,
        destination.place(),
        byte_offset,
        pointer,
        leaf_indices,
    )?;
    let declarations = &source
        .structural
        .as_ref()
        .ok_or_else(|| invalid())?
        .structural_types;
    if let LegalizedScalarInstructionKind::StructuralScalarFieldStore { path, field, .. } =
        &row.kind
    {
        if let Some(fragments) =
            crate::structural_inputs::structural_reference_input::scalar_field_fragments(
                destination.structural_type(),
                path,
                *field,
                declarations,
            )
        {
            // A fragmented update reads complete containers before preserving
            // sibling bits. Write-only authority cannot license that RMW.
            if matches!(&destination,
                abstract_operations::StructuralScalarStoreDestination::Parameter(parameter)
                    if parameter.access == terminal_psi::StructuralAccess::WriteOnlyBorrow)
                || reference_destination
                    && crate::selection::reference_input::referent_projection(
                        source,
                        destination.place(),
                        path,
                        terminal_psi::StructuralAccess::MutableBorrow,
                    )
                    .is_none()
            {
                return Err(invalid());
            }
            return super::fragmented_scalar::write(
                builder,
                row,
                destination.place(),
                address,
                byte_offset,
                fragments,
                leaf_indices,
                value.value,
            );
        }
    }
    let codec = match &row.kind {
        LegalizedScalarInstructionKind::StructuralScalarFieldStore { path, field, .. } => {
            crate::structural_inputs::structural_reference_input::scalar_field_codec(
                destination.structural_type(),
                path,
                *field,
                declarations,
            )
        }
        LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore { path, .. } => {
            crate::structural_inputs::structural_reference_input::projected_scalar_codec(
                destination.structural_type(),
                path,
                declarations,
            )
        }
        _ => None,
    }
    .ok_or_else(|| invalid())?;
    if u32::from(codec.byte_size()) != u32::from(byte_size) {
        return Err(invalid());
    }
    let canonical =
        runtime_abi::ScalarStorageCodec::from_scalar(value.scalar_type).ok_or_else(|| invalid())?;
    if codec != canonical || matches!(codec.byte_size(), 3 | 5 | 6 | 7) {
        let address = super::encoded_scalar::backing_address(
            builder,
            row,
            destination.place(),
            address,
            byte_offset,
        )?;
        let codec =
            super::encoded_scalar::constant_codec(builder, row, destination.place(), codec)?;
        return super::encoded_scalar::write(
            builder,
            row,
            destination.place(),
            address,
            codec,
            *value,
        );
    }
    let (address, displacement) = super::memory_pointer(
        builder,
        row,
        destination.place(),
        address,
        byte_offset,
        u32::from(byte_size),
    )?;
    super::runtime_address::footprint(
        builder,
        row,
        destination.place(),
        byte_offset,
        u32::from(byte_size),
        leaf_indices,
        super::runtime_address::Written::Scalar(value.value),
    )?;
    builder.emit_store(
        displacement,
        u8::try_from(byte_size).map_err(|_| invalid())?,
        address,
        register,
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: indices
                .iter()
                .map(|index| index.operand.value)
                .chain([value.value])
                .collect(),
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    Ok(())
}
