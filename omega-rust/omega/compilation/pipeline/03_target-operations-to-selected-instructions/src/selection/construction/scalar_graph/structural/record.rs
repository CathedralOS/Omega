//! Complete record storage is published only after all field writes.
use super::super::{IntegerValue, ValueId};
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    PlaceId, SelectedInstructionKind, SelectedInstructionProvenance, SelectedMemoryAccessRole,
    VirtualRegisterId, memory,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::invalid;
use crate::selection::construction::scalar_graph::structural::local_storage;
use crate::selection::construction::scalar_graph::structural::provenance;
use crate::selection::construction::scalar_graph::structural::transport_register;
use selected_instructions::{LocalStorageSlotId, SelectedLocalStorageSlot};
pub(super) fn establish(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let fields = crate::selection::record_input::fields(source, row).ok_or_else(|| invalid())?;
    let LegalizedScalarInstructionKind::EstablishRecord {
        result,
        fields: initializers,
        shape,
    } = &row.kind
    else {
        return Err(invalid());
    };
    let pointer = initialize_storage(row, builder, result.place, *shape)?;
    if shape.byte_size == 0 {
        builder.transport.pointers.push((result.place, pointer));
        return Ok(());
    }
    write_fields(
        source,
        row,
        builder,
        result.place,
        pointer,
        initializers,
        &fields,
    )?;
    builder.transport.pointers.push((result.place, pointer));
    Ok(())
}
/// Allocate the unpublished complete value and initialize every byte,
/// including padding that subsequent whole-value transfers may observe.
pub(super) fn initialize_storage(
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
    destination: PlaceId,
    shape: calling_conventions::ValueShape,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let slot = LocalStorageSlotId::Structural {
        operation: row.operation,
        place: destination,
    };
    builder
        .transport
        .local_slots
        .push(SelectedLocalStorageSlot {
            id: slot,
            // Even an empty record can be borrowed. Reserve an addressable
            // home without adding a byte to its semantic value or ABI shape.
            byte_size: shape.byte_size.max(1),
            alignment: shape.alignment,
        });
    let pointer = local_storage::address(builder, row, slot, 0, shape.byte_size.max(1), true)?;
    if shape.byte_size == 0 {
        return Ok(pointer);
    }
    // Initialize padding too: a later whole-value copy may transport every byte.
    let zero = transport_register(builder, destination, 0)?;
    builder.emit(
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(0),
        },
        builder.constraints.keys.materialize_i64,
        &[zero],
        provenance(row),
    )?;
    let mut offset = 0u32;
    while offset < shape.byte_size {
        let width = chunk(shape.byte_size - offset, offset, offset);
        store(
            builder,
            row,
            destination,
            pointer,
            offset,
            width,
            zero,
            Vec::new(),
            Vec::new(),
        )?;
        offset += u32::from(width);
    }
    Ok(pointer)
}
/// Initialize each owned field from its exact scalar or completed child storage.
/// Case construction uses the same writes after establishing its tag.
#[allow(clippy::too_many_arguments)]
pub(super) fn write_fields(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
    destination: PlaceId,
    pointer: VirtualRegisterId,
    initializers: &[terminal_psi::RecordFieldInitializer],
    fields: &[(
        u32,
        calling_conventions::ValueShape,
        Option<semantic_vocabulary::ScalarType>,
    )],
) -> Result<(), SelectedInstructionError> {
    if initializers.len() != fields.len() {
        return Err(invalid());
    }
    for (initializer, (offset, field_shape, expected_scalar)) in
        initializers.iter().zip(fields.iter().copied())
    {
        match &initializer.value {
            terminal_psi::RecordFieldValue::Scalar {
                value,
                range_obligation,
            } => {
                let (_, register, _, scalar) = builder.resolve(*value).ok_or_else(|| invalid())?;
                if Some(scalar) != expected_scalar
                    || crate::selection::scalar_call_abi::scalar_shape(scalar) != Some(field_shape)
                {
                    return Err(invalid());
                }
                let width = u8::try_from(field_shape.byte_size).map_err(|_| invalid())?;
                store(
                    builder,
                    row,
                    destination,
                    pointer,
                    offset,
                    width,
                    register,
                    vec![*value],
                    range_obligation.iter().copied().collect(),
                )?;
            }
            terminal_psi::RecordFieldValue::Structural(argument) => {
                write_structural_operand(
                    source,
                    row,
                    builder,
                    destination,
                    pointer,
                    argument,
                    offset,
                    field_shape,
                )?;
            }
        }
    }
    Ok(())
}

// A completed child can begin at a narrower-aligned field offset than its
// total size suggests. Both ends must use encodable scaled displacements;
// copying eight bytes merely because eight remain can select an impossible
// AArch64 store. Keep wide chunks when both offsets permit them, and never
// round the live extent up into a neighboring field.
pub(super) fn chunk(remaining: u32, source_offset: u32, destination_offset: u32) -> u8 {
    [8u8, 4, 2, 1]
        .into_iter()
        .find(|width| {
            let width = u32::from(*width);
            width <= remaining
                && source_offset.is_multiple_of(width)
                && destination_offset.is_multiple_of(width)
        })
        .unwrap_or(1)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn store(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    pointer: VirtualRegisterId,
    offset: u32,
    width: u8,
    value: VirtualRegisterId,
    values: Vec<ValueId>,
    obligations: Vec<semantic_vocabulary::ObligationId>,
) -> Result<(), SelectedInstructionError> {
    let (pointer, displacement) =
        super::memory_pointer(builder, row, place, pointer, offset, u32::from(width))?;
    memory(
        builder,
        row,
        place,
        offset,
        u32::from(width),
        SelectedMemoryAccessRole::WritePlace,
    )?;
    builder.emit(
        SelectedInstructionKind::Store {
            byte_offset: displacement,
            byte_size: width,
        },
        builder.constraints.keys.store.ok_or_else(|| invalid())?,
        &[pointer, value],
        SelectedInstructionProvenance {
            values,
            obligations,
            ..provenance(row)
        },
    )?;
    Ok(())
}

/// Copy one completed child into a constructor's unpublished destination.
#[allow(clippy::too_many_arguments)]
pub(super) fn write_structural_operand(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
    destination: PlaceId,
    pointer: VirtualRegisterId,
    argument: &terminal_psi::StructuralArgument,
    offset: u32,
    field_shape: calling_conventions::ValueShape,
) -> Result<(), SelectedInstructionError> {
    let source_offset = crate::selection::record_input::operand_offset(source, argument)
        .ok_or_else(|| invalid())?;
    if field_shape.byte_size == 0 {
        return Ok(());
    }
    // Once an owned input has addressable storage, later borrows
    // may update it. Copy its current bytes, not stale entry fragments.
    if let Some(parameter) = source
        .structural
        .as_ref()
        .and_then(|signature| {
            signature
                .parameters
                .iter()
                .find(|parameter| parameter.semantic.place == argument.place)
        })
        .filter(|_| {
            argument.path.is_empty()
                && !builder
                    .transport
                    .pointers
                    .iter()
                    .any(|(place, _)| *place == argument.place)
        })
    {
        if !crate::selection::aggregate_result_input::inline_argument_fragments(
            &parameter.target.placement,
        ) {
            return Err(invalid());
        }
        for location in &parameter.target.placement.locations {
            let (fragment_offset, width) = match location {
                calling_conventions::ValueLocation::Register {
                    value_byte_offset,
                    byte_size,
                    ..
                }
                | calling_conventions::ValueLocation::Stack {
                    value_byte_offset,
                    byte_size,
                    ..
                } => ((*value_byte_offset), *byte_size),
                _ => return Err(invalid()),
            };
            let value = builder
                .transport
                .fragments
                .iter()
                .find(|(place, byte_offset, _)| {
                    *place == argument.place && *byte_offset == fragment_offset
                })
                .map(|(_, _, value)| *value)
                .ok_or_else(|| invalid())?;
            memory(
                builder,
                row,
                destination,
                offset + fragment_offset,
                width,
                SelectedMemoryAccessRole::WritePlace,
            )?;
            super::super::aggregate_memory::store(
                builder,
                pointer,
                value,
                offset + fragment_offset,
                width,
            )?;
        }
        return Ok(());
    }
    let input = builder
        .transport
        .pointers
        .iter()
        .find(|(place, _)| *place == argument.place)
        .map(|(_, pointer)| *pointer)
        .ok_or_else(|| invalid())?;
    let mut cursor = 0u32;
    while cursor < field_shape.byte_size {
        let source_position = source_offset.checked_add(cursor).ok_or_else(|| invalid())?;
        let width = chunk(
            field_shape.byte_size - cursor,
            source_position,
            offset + cursor,
        );
        let (input, displacement) = super::memory_pointer(
            builder,
            row,
            argument.place,
            input,
            source_position,
            u32::from(width),
        )?;
        let value = transport_register(builder, argument.place, source_position)?;
        memory(
            builder,
            row,
            argument.place,
            source_position,
            u32::from(width),
            SelectedMemoryAccessRole::ReadPlace,
        )?;
        let (kind, key) = match width {
            8 => (
                SelectedInstructionKind::Load64 {
                    byte_offset: displacement,
                },
                builder.constraints.keys.load64,
            ),
            4 => (
                SelectedInstructionKind::Load32 {
                    byte_offset: displacement,
                },
                builder.constraints.keys.load32,
            ),
            2 => (
                SelectedInstructionKind::Load16 {
                    byte_offset: displacement,
                },
                builder.constraints.keys.load16,
            ),
            _ => (
                SelectedInstructionKind::Load8 {
                    byte_offset: displacement,
                },
                builder.constraints.keys.load8,
            ),
        };
        builder.emit(
            kind,
            key.ok_or_else(|| invalid())?,
            &[input, value],
            provenance(row),
        )?;
        store(
            builder,
            row,
            destination,
            pointer,
            offset + cursor,
            width,
            value,
            Vec::new(),
            Vec::new(),
        )?;
        cursor += u32::from(width);
    }
    Ok(())
}
