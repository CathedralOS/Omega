//! Complete record storage is published only after all field writes.
use super::*;
use selected_instructions::{LocalStorageSlotId, SelectedLocalStorageSlot};
pub(super) fn establish(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let fields = crate::selection::record_input::fields(source, row).ok_or_else(invalid)?;
    let LegalizedScalarInstructionKind::EstablishRecord {
        result,
        fields: initializers,
        shape,
    } = &row.kind
    else {
        return Err(invalid());
    };
    if shape.byte_size == 0 {
        builder.pending_provenance.operations.push(row.operation);
        builder
            .pending_provenance
            .fuel
            .extend(row.fuel.iter().cloned());
        return Ok(());
    }
    let slot = LocalStorageSlotId::Structural {
        operation: row.operation,
        place: result.place,
    };
    builder
        .transport
        .local_slots
        .push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: u32::from(shape.byte_size),
            alignment: shape.alignment,
        });
    let pointer = local_storage::address(builder, row, slot, 0, u32::from(shape.byte_size), true)?;
    // Initialize padding too: a later whole-value copy may transport every byte.
    let zero = transport_register(builder, result.place, 0)?;
    builder.emit(
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(0),
        },
        builder.constraints.keys.materialize_i64,
        &[zero],
        provenance(row),
    )?;
    let mut offset = 0u32;
    while offset < u32::from(shape.byte_size) {
        let width = chunk(u32::from(shape.byte_size) - offset);
        store(
            builder,
            row,
            result.place,
            pointer,
            offset,
            width,
            zero,
            Vec::new(),
            Vec::new(),
        )?;
        offset += u32::from(width);
    }
    for (initializer, (offset, field_shape, expected_scalar)) in initializers.iter().zip(fields) {
        match &initializer.value {
            terminal_psi::RecordFieldValue::Scalar {
                value,
                range_obligation,
            } => {
                let (_, register, _, scalar) = builder.resolve(*value).ok_or_else(invalid)?;
                if Some(scalar) != expected_scalar
                    || crate::selection::scalar_call_abi::scalar_shape(scalar) != Some(field_shape)
                {
                    return Err(invalid());
                }
                let width = u8::try_from(field_shape.byte_size).map_err(|_| invalid())?;
                store(
                    builder,
                    row,
                    result.place,
                    pointer,
                    offset,
                    width,
                    register,
                    vec![*value],
                    range_obligation.iter().copied().collect(),
                )?;
            }
            terminal_psi::RecordFieldValue::Structural(argument) => {
                if field_shape.byte_size == 0 {
                    continue;
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
                        !builder
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
                            } => (u32::from(*value_byte_offset), *byte_size),
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
                            .ok_or_else(invalid)?;
                        memory(
                            builder,
                            row,
                            result.place,
                            offset + fragment_offset,
                            u32::from(width),
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
                    continue;
                }
                let input = builder
                    .transport
                    .pointers
                    .iter()
                    .find(|(place, _)| *place == argument.place)
                    .map(|(_, pointer)| *pointer)
                    .ok_or_else(invalid)?;
                let mut cursor = 0u32;
                while cursor < u32::from(field_shape.byte_size) {
                    let width = chunk(u32::from(field_shape.byte_size) - cursor);
                    let value = transport_register(builder, argument.place, cursor)?;
                    memory(
                        builder,
                        row,
                        argument.place,
                        cursor,
                        u32::from(width),
                        SelectedMemoryAccessRole::ReadPlace,
                    )?;
                    let (kind, key) = match width {
                        8 => (
                            SelectedInstructionKind::Load64 {
                                byte_offset: cursor,
                            },
                            builder.constraints.keys.load64,
                        ),
                        4 => (
                            SelectedInstructionKind::Load32 {
                                byte_offset: cursor,
                            },
                            builder.constraints.keys.load32,
                        ),
                        2 => (
                            SelectedInstructionKind::Load16 {
                                byte_offset: cursor,
                            },
                            builder.constraints.keys.load16,
                        ),
                        _ => (
                            SelectedInstructionKind::Load8 {
                                byte_offset: cursor,
                            },
                            builder.constraints.keys.load8,
                        ),
                    };
                    builder.emit(
                        kind,
                        key.ok_or_else(invalid)?,
                        &[input, value],
                        provenance(row),
                    )?;
                    store(
                        builder,
                        row,
                        result.place,
                        pointer,
                        offset + cursor,
                        width,
                        value,
                        Vec::new(),
                        Vec::new(),
                    )?;
                    cursor += u32::from(width);
                }
            }
        }
    }
    builder.transport.pointers.push((result.place, pointer));
    Ok(())
}
fn chunk(remaining: u32) -> u8 {
    [8u8, 4, 2, 1]
        .into_iter()
        .find(|width| u32::from(*width) <= remaining)
        .unwrap_or(1)
}
#[allow(clippy::too_many_arguments)]
fn store(
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
            byte_offset: offset,
            byte_size: width,
        },
        builder.constraints.keys.store.ok_or_else(invalid)?,
        &[pointer, value],
        SelectedInstructionProvenance {
            values,
            obligations,
            ..provenance(row)
        },
    )?;
    Ok(())
}
