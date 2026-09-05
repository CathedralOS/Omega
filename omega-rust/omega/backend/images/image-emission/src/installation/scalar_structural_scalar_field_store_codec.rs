//! Canonical transport for bounded ordered direct mutable-self scalar-store records.

use machine_code::ScalarStructuralScalarFieldStoreRecord;
use semantic_vocabulary::{OperationId, StructuralFieldId};
use target_operations::TargetScalarImmediate;

use super::{
    InstallationError, Reader,
    boundary_result_scalar_codec::{
        decode_boundary_result_scalar_type, encode_boundary_result_scalar_type,
    },
    decode_boolean,
    internal_unit_scalar_call_codec::{decode_offset, encode_offset},
    push_u32, push_u64,
    unit_scalar_codec::{
        decode_integer_type, decode_integer_value, encode_integer_type, encode_integer_value,
    },
    unit_structural_scalar_field_store_codec::{
        decode_destination, decode_path, encode_destination, encode_path,
    },
    value_placement_codec::{decode_direct_placement, encode_direct_placement},
};

pub(super) fn encode_scalar_structural_scalar_field_stores(
    bytes: &mut Vec<u8>,
    stores: &[ScalarStructuralScalarFieldStoreRecord],
) -> Result<(), InstallationError> {
    if stores.len() > 3 {
        return Err(InstallationError::TooManyScalarStructuralScalarFieldStores);
    }
    push_u32(
        bytes,
        u32::try_from(stores.len())
            .map_err(|_| InstallationError::TooManyScalarStructuralScalarFieldStores)?,
    );
    for store in stores {
        push_u64(bytes, store.psi_operation.get());
        encode_destination(bytes, &store.destination)?;
        encode_path(bytes, &store.path)?;
        push_u64(bytes, store.field.get());
        encode_direct_placement(bytes, &store.destination_placement)?;
        push_u32(bytes, store.field_byte_offset);
        push_u64(bytes, store.defining_operation.get());
        push_u64(bytes, store.source_value.get());
        match store.immediate {
            TargetScalarImmediate::Boolean(value) => {
                bytes.extend_from_slice(&[1, u8::from(value), 0, 0]);
            }
            TargetScalarImmediate::Integer { scalar_type, value } => {
                bytes.extend_from_slice(&[2, 0, 0, 0]);
                encode_integer_type(bytes, scalar_type)?;
                encode_integer_value(bytes, value);
            }
        }
        push_u64(bytes, store.return_operation.get());
        push_u64(bytes, store.return_source_value.get());
        push_u64(bytes, store.return_field.get());
        push_u32(bytes, store.return_field_byte_offset);
        encode_boundary_result_scalar_type(bytes, store.return_scalar_type);
        encode_offset(bytes, store.operation_ordinal)?;
        encode_offset(bytes, store.code_offset)?;
        encode_offset(bytes, store.byte_count)?;
        push_u32(
            bytes,
            u32::try_from(store.bytes.len())
                .map_err(|_| InstallationError::TooManyUnitStructuralScalarFieldStoreBytes)?,
        );
        bytes.extend_from_slice(&store.bytes);
    }
    Ok(())
}

pub(super) fn decode_scalar_structural_scalar_field_stores(
    reader: &mut Reader<'_>,
) -> Result<Vec<ScalarStructuralScalarFieldStoreRecord>, InstallationError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyScalarStructuralScalarFieldStores)?;
    if count > 3 {
        return Err(InstallationError::TooManyScalarStructuralScalarFieldStores);
    }
    if count > reader.remaining() / 112 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut stores = Vec::with_capacity(count);
    for _ in 0..count {
        let psi_operation = OperationId::new(reader.u64()?)
            .ok_or(InstallationError::ZeroInstalledScalarIdentity)?;
        let destination = decode_destination(reader)?;
        let path = decode_path(reader)?;
        let field = StructuralFieldId::new(reader.u64()?)
            .ok_or(InstallationError::ZeroInstalledScalarIdentity)?;
        let destination_placement = decode_direct_placement(reader)?;
        let field_byte_offset = reader.u32()?;
        let defining_operation = OperationId::new(reader.u64()?)
            .ok_or(InstallationError::ZeroInstalledScalarIdentity)?;
        let source_value = semantic_vocabulary::ValueId::new(reader.u64()?)
            .ok_or(InstallationError::ZeroInstalledScalarIdentity)?;
        let immediate = match reader.u8()? {
            1 => {
                let value = decode_boolean(reader.u8()?)?;
                if reader.take(2)? != [0; 2] {
                    return Err(InstallationError::NonzeroReservedField);
                }
                TargetScalarImmediate::Boolean(value)
            }
            2 => {
                if reader.take(3)? != [0; 3] {
                    return Err(InstallationError::NonzeroReservedField);
                }
                TargetScalarImmediate::Integer {
                    scalar_type: decode_integer_type(reader)?,
                    value: decode_integer_value(reader)?,
                }
            }
            _ => return Err(InstallationError::NonCanonicalEncoding),
        };
        let return_operation = OperationId::new(reader.u64()?)
            .ok_or(InstallationError::ZeroInstalledScalarIdentity)?;
        let return_source_value = semantic_vocabulary::ValueId::new(reader.u64()?)
            .ok_or(InstallationError::ZeroInstalledScalarIdentity)?;
        let return_field = StructuralFieldId::new(reader.u64()?)
            .ok_or(InstallationError::ZeroInstalledScalarIdentity)?;
        let return_field_byte_offset = reader.u32()?;
        let return_scalar_type = decode_boundary_result_scalar_type(reader)?;
        let operation_ordinal = decode_offset(reader)?;
        let code_offset = decode_offset(reader)?;
        let byte_count = decode_offset(reader)?;
        let bytes_len = usize::try_from(reader.u32()?)
            .map_err(|_| InstallationError::TooManyUnitStructuralScalarFieldStoreBytes)?;
        let store_bytes = reader.take(bytes_len)?.to_vec();
        stores.push(ScalarStructuralScalarFieldStoreRecord {
            psi_operation,
            destination,
            path,
            field,
            destination_placement,
            field_byte_offset,
            defining_operation,
            source_value,
            immediate,
            return_operation,
            return_source_value,
            return_field,
            return_field_byte_offset,
            return_scalar_type,
            operation_ordinal,
            code_offset,
            byte_count,
            bytes: store_bytes,
        });
    }
    Ok(stores)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_store_count_is_bounded_canonically() {
        let mut empty = Reader::new(&[0, 0, 0, 0]);
        assert!(
            decode_scalar_structural_scalar_field_stores(&mut empty)
                .expect("empty scalar-store roster")
                .is_empty()
        );

        let mut four = Reader::new(&[4, 0, 0, 0]);
        assert_eq!(
            decode_scalar_structural_scalar_field_stores(&mut four),
            Err(InstallationError::TooManyScalarStructuralScalarFieldStores)
        );
    }
}
