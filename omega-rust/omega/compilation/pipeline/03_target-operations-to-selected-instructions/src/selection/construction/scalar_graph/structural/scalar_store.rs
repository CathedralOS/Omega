//! Exact writes through original borrowed pointers, owned result homes, or an
//! owned arrival's block home. A path through runtime-selected elements scales
//! each selector into the address first (`runtime_address`); the store then
//! applies the static offset.
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    SelectedInstructionKind, SelectedInstructionProvenance,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::invalid;
use crate::structural_inputs::structural_reference_input::PointerSlot;
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
                let reference =
                    crate::selection::reference_input::store_destination(source, destination, path);
                let (root, suffix) =
                    reference.unwrap_or((destination.structural_type(), path.as_slice()));
                let (offset, size, elements) =
                    crate::structural_inputs::structural_reference_input::store(
                        root,
                        suffix,
                        *field,
                        value.scalar_type,
                        &signature.structural_types,
                    )
                    .ok_or_else(|| invalid())?;
                if (offset, size) != (*byte_offset, *byte_size)
                    || !runtime_indices_match(indices, &elements)
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
                    reference
                        .map(|_| PointerSlot {
                            byte_offset: 0,
                            indices: 0..0,
                        })
                        .into_iter()
                        .collect::<Vec<_>>(),
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
                if !signature.entry_claims.is_empty()
                    || (geometry.byte_offset, geometry.byte_size) != (*byte_offset, *byte_size)
                    || !runtime_indices_match(indices, &geometry.elements)
                {
                    return Err(invalid());
                }
                (
                    destination.clone(),
                    value,
                    *byte_offset,
                    *byte_size,
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
        // An owned arrival's field lands in its own block home, the slot
        // whose address `block_entry` placed in the transport pointers.
        abstract_operations::StructuralScalarStoreDestination::OwnedBlockParameter {
            block,
            parameter,
        } => {
            matches!(
                row.kind,
                LegalizedScalarInstructionKind::StructuralScalarFieldStore { .. }
            ) && crate::selection::owned_block_home_input::store_destination(
                source, *block, parameter,
            )
        }
    };
    if row.result.is_some() || !declared {
        return Err(invalid());
    }
    let mut pointer = if pointer_slots.is_empty() {
        super::runtime_address::projection_base(
            source,
            builder,
            row,
            destination.place(),
            byte_offset,
        )?
        .ok_or_else(|| invalid())?
    } else {
        super::place_pointer(builder, row, destination.place(), 0)?
    };
    // Every reference crossing ends its own contiguous address region. Preserve
    // each pointer load before applying the following region's offset or indices.
    let mut leaf_indices = indices.as_slice();
    for slot in pointer_slots {
        let slot_indices = indices.get(slot.indices.clone()).ok_or_else(invalid)?;
        let address = super::runtime_address::scale(
            builder,
            row,
            destination.place(),
            slot.byte_offset,
            pointer,
            slot_indices,
        )?;
        let (address, displacement) = super::memory_pointer(
            builder,
            row,
            destination.place(),
            address,
            slot.byte_offset,
            8,
        )?;
        super::runtime_address::footprint(
            builder,
            row,
            destination.place(),
            slot.byte_offset,
            8,
            slot_indices,
            super::runtime_address::Written::None,
        )?;
        let loaded = super::transport_register(builder, destination.place(), slot.byte_offset)?;
        builder.emit(
            SelectedInstructionKind::Load64 {
                byte_offset: displacement,
            },
            builder.constraints.keys.load64.ok_or_else(invalid)?,
            &[address, loaded],
            super::provenance(row),
        )?;
        pointer = loaded;
        leaf_indices = indices.get(slot.indices.end..).ok_or_else(invalid)?;
    }
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
    builder.emit(
        SelectedInstructionKind::Store {
            byte_offset: displacement,
            byte_size,
        },
        builder.constraints.keys.store.ok_or_else(|| invalid())?,
        &[address, register],
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
