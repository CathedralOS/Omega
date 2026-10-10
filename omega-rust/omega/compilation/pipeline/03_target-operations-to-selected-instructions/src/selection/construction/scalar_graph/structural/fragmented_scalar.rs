//! Read and update an admitted fragmented scalar in its original containers.
//! A container is accessed as exact bytes, so unaligned and odd-sized storage
//! uses the same operation as naturally aligned storage. Writes preserve every
//! sibling bit by masked RMW; their physical reads and writes remain visible.
//! The envelope is only a bounds summary, never a contiguous scalar address.

use super::{
    Builder, LegalizedScalarInstruction, PlaceId, SelectedInstructionKind, VirtualRegisterId,
    invalid, provenance, transport_register,
};
use crate::SelectedInstructionError;
use legalized_operations::LegalizedRuntimeIndexOperand;
use semantic_vocabulary::{
    AuthoredBitFragment, AuthoredBitFragments, IntegerValue, ScalarType, ValueId,
};

fn constant(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    value: u64,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let output = transport_register(builder, place, 0)?;
    builder.emit_materialize_i64(
        IntegerValue::Unsigned(u128::from(value)),
        output,
        provenance(row),
    )?;
    Ok(output)
}

fn bitwise(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    kind: SelectedInstructionKind,
    left: VirtualRegisterId,
    right: VirtualRegisterId,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let output = transport_register(builder, place, 0)?;
    builder.emit(
        kind,
        builder.constraints.keys.subtract_i64,
        &[left, right, output],
        provenance(row),
    )?;
    Ok(output)
}

fn shift(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    input: VirtualRegisterId,
    count: u64,
    left: bool,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    if count == 0 {
        return Ok(input);
    }
    if count >= 64 {
        return Err(invalid());
    }
    let count = constant(builder, row, place, count)?;
    let output = transport_register(builder, place, 0)?;
    builder.emit(
        if left {
            SelectedInstructionKind::WrappingShiftLeftI64
        } else {
            SelectedInstructionKind::WrappingShiftRightU64
        },
        builder.constraints.keys.shift_i64,
        &[input, count, output],
        provenance(row),
    )?;
    Ok(output)
}

fn mask(width: u64) -> Result<u64, SelectedInstructionError> {
    match width {
        1..=63 => Ok((1_u64 << width) - 1),
        64 => Ok(u64::MAX),
        _ => Err(invalid()),
    }
}

fn container_width(fragment: &AuthoredBitFragment) -> Result<u32, SelectedInstructionError> {
    if !(8..=64).contains(&fragment.container_width_bits)
        || !fragment.container_width_bits.is_multiple_of(8)
        || fragment
            .destination_lsb
            .checked_add(fragment.width)
            .is_none_or(|end| end > fragment.container_width_bits)
        || fragment
            .source_lsb
            .checked_add(fragment.width)
            .is_none_or(|end| end > 64)
    {
        return Err(invalid());
    }
    u32::try_from(fragment.container_width_bits / 8).map_err(|_| invalid())
}

fn prefix_offset(
    envelope_offset: u32,
    fragments: &AuthoredBitFragments,
) -> Result<u32, SelectedInstructionError> {
    let (first, _) = fragments.byte_envelope().ok_or_else(invalid)?;
    envelope_offset
        .checked_sub(u32::try_from(first).map_err(|_| invalid())?)
        .ok_or_else(invalid)
}

fn container_offset(
    prefix: u32,
    fragment: &AuthoredBitFragment,
) -> Result<u32, SelectedInstructionError> {
    prefix
        .checked_add(u32::try_from(fragment.container_byte_offset).map_err(|_| invalid())?)
        .ok_or_else(invalid)
}

#[allow(clippy::too_many_arguments)]
fn load_container(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    pointer: VirtualRegisterId,
    offset: u32,
    width: u32,
    indices: &[LegalizedRuntimeIndexOperand],
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let mut container = constant(builder, row, place, 0)?;
    for position in 0..width {
        let byte_offset = offset.checked_add(position).ok_or_else(invalid)?;
        let (address, displacement) =
            super::memory_pointer(builder, row, place, pointer, byte_offset, 1)?;
        let byte = transport_register(builder, place, byte_offset)?;
        super::runtime_address::footprint(
            builder,
            row,
            place,
            byte_offset,
            1,
            indices,
            super::runtime_address::Written::None,
        )?;
        builder.emit(
            SelectedInstructionKind::Load8 {
                byte_offset: displacement,
            },
            builder.constraints.keys.load8.ok_or_else(invalid)?,
            &[address, byte],
            provenance(row),
        )?;
        let byte = shift(builder, row, place, byte, u64::from(position) * 8, true)?;
        container = bitwise(
            builder,
            row,
            place,
            SelectedInstructionKind::BitwiseOrI64,
            container,
            byte,
        )?;
    }
    Ok(container)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn read(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    pointer: VirtualRegisterId,
    envelope_offset: u32,
    fragments: &AuthoredBitFragments,
    indices: &[LegalizedRuntimeIndexOperand],
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let definition = row.result.ok_or_else(invalid)?;
    if !matches!(
        definition.scalar_type,
        ScalarType::Boolean | ScalarType::Integer(_)
    ) {
        return Err(invalid());
    }
    let prefix = prefix_offset(envelope_offset, fragments)?;
    let mut value = constant(builder, row, place, 0)?;
    for fragment in fragments.fragments() {
        let width = container_width(fragment)?;
        let offset = container_offset(prefix, fragment)?;
        let container = load_container(builder, row, place, pointer, offset, width, indices)?;
        let payload = shift(
            builder,
            row,
            place,
            container,
            fragment.destination_lsb,
            false,
        )?;
        let source_mask = constant(builder, row, place, mask(fragment.width)?)?;
        let payload = bitwise(
            builder,
            row,
            place,
            SelectedInstructionKind::BitwiseAndI64,
            payload,
            source_mask,
        )?;
        let payload = shift(builder, row, place, payload, fragment.source_lsb, true)?;
        value = bitwise(
            builder,
            row,
            place,
            SelectedInstructionKind::BitwiseOrI64,
            value,
            payload,
        )?;
    }
    let output = builder.register(
        definition.value,
        definition.definition_site,
        definition.scalar_type,
    )?;
    builder.emit(
        crate::selection::scalar_call_abi::integer_carrier_normalization(definition.scalar_type),
        builder.constraints.keys.copy_i64,
        &[value, output],
        provenance(row),
    )?;
    Ok(output)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn write(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    pointer: VirtualRegisterId,
    envelope_offset: u32,
    fragments: &AuthoredBitFragments,
    indices: &[LegalizedRuntimeIndexOperand],
    value: ValueId,
) -> Result<(), SelectedInstructionError> {
    let (_, value_register, _, scalar) = builder.resolve(value).ok_or_else(invalid)?;
    if !matches!(scalar, ScalarType::Boolean | ScalarType::Integer(_)) {
        return Err(invalid());
    }
    let prefix = prefix_offset(envelope_offset, fragments)?;
    for fragment in fragments.fragments() {
        let width = container_width(fragment)?;
        let offset = container_offset(prefix, fragment)?;
        let container = load_container(builder, row, place, pointer, offset, width, indices)?;
        let source_mask_bits = mask(fragment.width)?;
        let destination_mask = source_mask_bits
            .checked_shl(u32::try_from(fragment.destination_lsb).map_err(|_| invalid())?)
            .ok_or_else(invalid)?;
        let keep = constant(builder, row, place, !destination_mask)?;
        let preserved = bitwise(
            builder,
            row,
            place,
            SelectedInstructionKind::BitwiseAndI64,
            container,
            keep,
        )?;
        let payload = shift(
            builder,
            row,
            place,
            value_register,
            fragment.source_lsb,
            false,
        )?;
        let source_mask = constant(builder, row, place, source_mask_bits)?;
        let payload = bitwise(
            builder,
            row,
            place,
            SelectedInstructionKind::BitwiseAndI64,
            payload,
            source_mask,
        )?;
        let payload = shift(builder, row, place, payload, fragment.destination_lsb, true)?;
        let merged = bitwise(
            builder,
            row,
            place,
            SelectedInstructionKind::BitwiseOrI64,
            preserved,
            payload,
        )?;
        for position in 0..width {
            let byte_offset = offset.checked_add(position).ok_or_else(invalid)?;
            let payload = shift(builder, row, place, merged, u64::from(position) * 8, false)?;
            let (address, displacement) =
                super::memory_pointer(builder, row, place, pointer, byte_offset, 1)?;
            super::runtime_address::footprint(
                builder,
                row,
                place,
                byte_offset,
                1,
                indices,
                super::runtime_address::Written::Scalar(value),
            )?;
            let mut operation_provenance = provenance(row);
            if !operation_provenance.values.contains(&value) {
                operation_provenance.values.push(value);
            }
            builder.emit_store(displacement, 1, address, payload, operation_provenance)?;
        }
    }
    Ok(())
}
