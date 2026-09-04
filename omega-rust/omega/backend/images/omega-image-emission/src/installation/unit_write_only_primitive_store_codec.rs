//! Canonical transport for whole-root non-observing primitive-store rows.

use omega_machine_code::{
    UnitScalarParameterLocationRecord, UnitWriteOnlyPrimitiveStoreRecord,
    UnitWriteOnlyPrimitiveStoreSourceRecord,
};
use psi_core::{OperationId, ScalarType, StructuralTypeId, ValueId};
use psi_terminal::{StructuralTypeDeclaration, StructuralTypeShape};

use super::{
    InstallationError, Reader,
    boundary_result_scalar_codec::{
        decode_boundary_result_scalar_type, encode_boundary_result_scalar_type,
    },
    decode_boolean,
    internal_unit_scalar_call_codec::{decode_offset, encode_offset},
    push_u32, push_u64,
    structural_scalar_codec::{decode_identity, encode_identity},
    unit_scalar_codec::{
        decode_integer_type, decode_integer_value, decode_scalar_type, decode_unit_scalar_home,
        encode_integer_type, encode_integer_value, encode_scalar_type, encode_unit_scalar_home,
    },
    unit_structural_scalar_field_store_codec::{decode_destination, encode_destination},
    value_placement_codec::{
        decode_direct_placement, decode_register, encode_direct_placement, register_tag,
    },
};

pub(super) fn encode_unit_write_only_primitive_stores(
    bytes: &mut Vec<u8>,
    stores: &[UnitWriteOnlyPrimitiveStoreRecord],
) -> Result<(), InstallationError> {
    push_u32(
        bytes,
        u32::try_from(stores.len())
            .map_err(|_| InstallationError::TooManyUnitWriteOnlyPrimitiveStores)?,
    );
    for store in stores {
        push_u64(bytes, store.psi_operation.get());
        encode_destination(bytes, &store.destination)?;
        push_u64(bytes, store.destination_type.id.get());
        encode_identity(bytes, &store.destination_type.identity)?;
        let StructuralTypeShape::PrimitiveScalar(scalar_type) = store.destination_type.shape else {
            return Err(InstallationError::InvalidStructuralTypeShape);
        };
        encode_boundary_result_scalar_type(bytes, scalar_type);
        encode_direct_placement(bytes, &store.destination_placement)?;
        encode_source(bytes, store.source)?;
        push_u32(bytes, store.parameter_home_byte_offset);
        bytes.push(u8::from(store.parameter_home_indirect));
        bytes.extend_from_slice(&[0; 3]);
        encode_offset(bytes, store.operation_ordinal)?;
        encode_offset(bytes, store.code_offset)?;
        encode_offset(bytes, store.byte_count)?;
        push_u32(
            bytes,
            u32::try_from(store.bytes.len())
                .map_err(|_| InstallationError::TooManyUnitWriteOnlyPrimitiveStoreBytes)?,
        );
        bytes.extend_from_slice(&store.bytes);
    }
    Ok(())
}

pub(super) fn decode_unit_write_only_primitive_stores(
    reader: &mut Reader<'_>,
) -> Result<Vec<UnitWriteOnlyPrimitiveStoreRecord>, InstallationError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyUnitWriteOnlyPrimitiveStores)?;
    if count > reader.remaining() / 96 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut stores = Vec::with_capacity(count);
    for _ in 0..count {
        let psi_operation = OperationId::new(reader.u64()?)
            .ok_or(InstallationError::ZeroInstalledScalarIdentity)?;
        let destination = decode_destination(reader)?;
        let destination_type_id = StructuralTypeId::new(reader.u64()?)
            .ok_or(InstallationError::ZeroInstalledScalarIdentity)?;
        let destination_type_identity = decode_identity(reader)?;
        let destination_scalar_type = decode_boundary_result_scalar_type(reader)?;
        if !matches!(
            destination_scalar_type,
            ScalarType::Integer(_) | ScalarType::Boolean | ScalarType::IeeeFloat(_)
        ) {
            return Err(InstallationError::InvalidStructuralTypeShape);
        }
        let destination_placement = decode_direct_placement(reader)?;
        let source = decode_source(reader)?;
        let parameter_home_byte_offset = reader.u32()?;
        let parameter_home_indirect = decode_boolean(reader.u8()?)?;
        if reader.take(3)? != [0; 3] {
            return Err(InstallationError::NonzeroReservedField);
        }
        let operation_ordinal = decode_offset(reader)?;
        let code_offset = decode_offset(reader)?;
        let byte_count = decode_offset(reader)?;
        let bytes_len = usize::try_from(reader.u32()?)
            .map_err(|_| InstallationError::TooManyUnitWriteOnlyPrimitiveStoreBytes)?;
        let store_bytes = reader.take(bytes_len)?.to_vec();
        stores.push(UnitWriteOnlyPrimitiveStoreRecord {
            psi_operation,
            destination,
            destination_type: StructuralTypeDeclaration {
                id: destination_type_id,
                identity: destination_type_identity,
                shape: StructuralTypeShape::PrimitiveScalar(destination_scalar_type),
            },
            destination_placement,
            source,
            parameter_home_byte_offset,
            parameter_home_indirect,
            operation_ordinal,
            code_offset,
            byte_count,
            bytes: store_bytes,
        });
    }
    Ok(stores)
}

fn encode_source(
    bytes: &mut Vec<u8>,
    source: UnitWriteOnlyPrimitiveStoreSourceRecord,
) -> Result<(), InstallationError> {
    match source {
        UnitWriteOnlyPrimitiveStoreSourceRecord::Parameter {
            parameter_index,
            source_value,
            scalar_type,
            location,
        } => {
            bytes.extend_from_slice(&[4, 0, 0, 0]);
            push_u32(bytes, parameter_index);
            push_u64(bytes, source_value.get());
            encode_scalar_type(bytes, scalar_type)?;
            match location {
                UnitScalarParameterLocationRecord::Register(register) => {
                    bytes.push(0);
                    bytes.push(register_tag(register)?);
                    bytes.extend_from_slice(&[0; 2]);
                    push_u32(bytes, 0);
                }
                UnitScalarParameterLocationRecord::IncomingStack { byte_offset } => {
                    bytes.extend_from_slice(&[1, 0, 0, 0]);
                    push_u32(bytes, byte_offset);
                }
            }
        }
        UnitWriteOnlyPrimitiveStoreSourceRecord::IntegerImmediate {
            defining_operation,
            source_value,
            scalar_type,
            value,
        } => {
            bytes.extend_from_slice(&[1, 0, 0, 0]);
            push_u64(bytes, defining_operation.get());
            push_u64(bytes, source_value.get());
            encode_integer_type(bytes, scalar_type)?;
            encode_integer_value(bytes, value);
        }
        UnitWriteOnlyPrimitiveStoreSourceRecord::BooleanImmediate {
            defining_operation,
            source_value,
            value,
            definition_ordinal,
        } => {
            bytes.extend_from_slice(&[2, 0, 0, 0]);
            push_u64(bytes, defining_operation.get());
            push_u64(bytes, source_value.get());
            encode_offset(bytes, definition_ordinal)?;
            bytes.push(u8::from(value));
            bytes.extend_from_slice(&[0; 7]);
        }
        UnitWriteOnlyPrimitiveStoreSourceRecord::IeeeFloatImmediate {
            defining_operation,
            source_value,
            value,
            definition_ordinal,
        } => {
            bytes.extend_from_slice(&[3, 0, 0, 0]);
            push_u64(bytes, defining_operation.get());
            push_u64(bytes, source_value.get());
            encode_offset(bytes, definition_ordinal)?;
            let (format, bits) = match value {
                psi_core::IeeeFloatValue::Binary32(bits) => (1, u64::from(bits)),
                psi_core::IeeeFloatValue::Binary64(bits) => (2, bits),
            };
            bytes.push(format);
            bytes.extend_from_slice(&[0; 7]);
            push_u64(bytes, bits);
        }
        UnitWriteOnlyPrimitiveStoreSourceRecord::Home(home) => {
            bytes.extend_from_slice(&[5, 0, 0, 0]);
            encode_unit_scalar_home(bytes, home)?;
        }
    }
    Ok(())
}

fn decode_source(
    reader: &mut Reader<'_>,
) -> Result<UnitWriteOnlyPrimitiveStoreSourceRecord, InstallationError> {
    let tag = reader.u8()?;
    if reader.take(3)? != [0; 3] {
        return Err(InstallationError::NonzeroReservedField);
    }
    match tag {
        1 => Ok(UnitWriteOnlyPrimitiveStoreSourceRecord::IntegerImmediate {
            defining_operation: OperationId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroInstalledScalarIdentity)?,
            source_value: ValueId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroInstalledScalarIdentity)?,
            scalar_type: decode_integer_type(reader)?,
            value: decode_integer_value(reader)?,
        }),
        2 => {
            let defining_operation = OperationId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroInstalledScalarIdentity)?;
            let source_value = ValueId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroInstalledScalarIdentity)?;
            let definition_ordinal = decode_offset(reader)?;
            let value = decode_boolean(reader.u8()?)?;
            if reader.take(7)? != [0; 7] {
                return Err(InstallationError::NonzeroReservedField);
            }
            Ok(UnitWriteOnlyPrimitiveStoreSourceRecord::BooleanImmediate {
                defining_operation,
                source_value,
                value,
                definition_ordinal,
            })
        }
        3 => {
            let defining_operation = OperationId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroInstalledScalarIdentity)?;
            let source_value = ValueId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroInstalledScalarIdentity)?;
            let definition_ordinal = decode_offset(reader)?;
            let format = reader.u8()?;
            if reader.take(7)? != [0; 7] {
                return Err(InstallationError::NonzeroReservedField);
            }
            let bits = reader.u64()?;
            let value = match format {
                1 => psi_core::IeeeFloatValue::Binary32(
                    u32::try_from(bits).map_err(|_| InstallationError::NonzeroReservedField)?,
                ),
                2 => psi_core::IeeeFloatValue::Binary64(bits),
                tag => return Err(InstallationError::InvalidIeeeFloatFormatTag(tag)),
            };
            Ok(
                UnitWriteOnlyPrimitiveStoreSourceRecord::IeeeFloatImmediate {
                    defining_operation,
                    source_value,
                    value,
                    definition_ordinal,
                },
            )
        }
        4 => {
            let parameter_index = reader.u32()?;
            let source_value = ValueId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroInstalledScalarIdentity)?;
            let scalar_type = decode_scalar_type(reader)?;
            let location_tag = reader.u8()?;
            let register_tag_byte = reader.u8()?;
            if reader.take(2)? != [0; 2] {
                return Err(InstallationError::NonzeroReservedField);
            }
            let byte_offset = reader.u32()?;
            let location = match location_tag {
                0 if byte_offset == 0 => {
                    UnitScalarParameterLocationRecord::Register(decode_register(register_tag_byte)?)
                }
                1 if register_tag_byte == 0 => {
                    UnitScalarParameterLocationRecord::IncomingStack { byte_offset }
                }
                _ => {
                    return Err(InstallationError::InvalidInstalledScalarSourceTag(
                        location_tag,
                    ));
                }
            };
            Ok(UnitWriteOnlyPrimitiveStoreSourceRecord::Parameter {
                parameter_index,
                source_value,
                scalar_type,
                location,
            })
        }
        5 => Ok(UnitWriteOnlyPrimitiveStoreSourceRecord::Home(
            decode_unit_scalar_home(reader)?,
        )),
        tag => Err(InstallationError::InvalidInstalledScalarSourceTag(tag)),
    }
}
