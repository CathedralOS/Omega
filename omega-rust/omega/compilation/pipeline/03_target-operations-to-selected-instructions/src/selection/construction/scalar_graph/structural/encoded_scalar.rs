//! Access original scalar backing through its transported storage codec.
//! The selected operation owns exact runtime width; semantic carrier width is
//! not a memory footprint and cannot supply the missing descriptor geometry.

use super::{
    Builder, LegalizedScalarInstruction, SelectedInstructionKind, VirtualRegisterId, invalid,
    provenance, transport_register,
};
use crate::SelectedInstructionError;
use semantic_vocabulary::{PlaceId, ScalarType};

pub(super) fn backing_address(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    pointer: VirtualRegisterId,
    byte_offset: u32,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    if byte_offset == 0 {
        return Ok(pointer);
    }
    let address = transport_register(builder, place, byte_offset)?;
    super::transport_at_byte_offset(builder, pointer, address, byte_offset, provenance(row))
}

pub(super) fn constant_codec(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    codec: runtime_abi::ScalarStorageCodec,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let output = transport_register(builder, place, 8)?;
    builder.emit(
        SelectedInstructionKind::MaterializeI64 {
            value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(
                codec.encode().ok_or(invalid())?,
            )),
        },
        builder.constraints.keys.materialize_i64,
        &[output],
        provenance(row),
    )?;
    Ok(output)
}

pub(super) fn read(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    address: VirtualRegisterId,
    codec: VirtualRegisterId,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let definition = row.result.ok_or(invalid())?;
    if !matches!(
        definition.scalar_type,
        ScalarType::Integer(_) | ScalarType::Boolean | ScalarType::IeeeFloat(_)
    ) {
        return Err(invalid());
    }
    // The decoded payload is already the scalar graph's value: IEEE values
    // travel as their bits in the general class, exactly as an exact-width
    // load leaves them. Float registers appear only at an ABI transfer.
    let output = builder.register(
        definition.value,
        definition.definition_site,
        definition.scalar_type,
    )?;
    let cursor = transport_register(builder, place, 0)?;
    let count = transport_register(builder, place, 8)?;
    let byte = transport_register(builder, place, 0)?;
    builder.emit(
        SelectedInstructionKind::LoadEncodedScalar,
        builder
            .constraints
            .keys
            .load_encoded_scalar
            .ok_or(invalid())?,
        &[address, codec, cursor, count, byte, output],
        provenance(row),
    )?;
    Ok(output)
}

pub(super) fn write(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    address: VirtualRegisterId,
    codec: VirtualRegisterId,
    value: abstract_operations::AbstractResult,
) -> Result<(), SelectedInstructionError> {
    let (_, value_register, _, scalar) = builder.resolve(value.value).ok_or(invalid())?;
    if scalar != value.scalar_type
        || !matches!(
            scalar,
            ScalarType::Integer(_) | ScalarType::Boolean | ScalarType::IeeeFloat(_)
        )
    {
        return Err(invalid());
    }
    let mut operation_provenance = provenance(row);
    if !operation_provenance.values.contains(&value.value) {
        operation_provenance.values.push(value.value);
    }
    // An IEEE value register already holds the bits the codec stores.
    let cursor = transport_register(builder, place, 0)?;
    let count = transport_register(builder, place, 8)?;
    let payload = transport_register(builder, place, 0)?;
    builder.emit(
        SelectedInstructionKind::StoreEncodedScalar,
        builder
            .constraints
            .keys
            .store_encoded_scalar
            .ok_or(invalid())?,
        &[address, codec, value_register, cursor, count, payload],
        operation_provenance,
    )?;
    Ok(())
}
