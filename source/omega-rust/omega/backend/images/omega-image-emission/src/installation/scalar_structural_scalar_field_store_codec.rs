//! Canonical transport for the direct mutable-self scalar-store record.

use omega_machine_code::{
    InternalUnitScalarArgumentSourceRecord, ScalarStructuralScalarFieldStoreRecord,
};
use psi_core::{OperationId, StructuralFieldId};

use super::{
    InstallationError, Reader,
    internal_unit_scalar_call_codec::{
        decode_argument_source, decode_offset, encode_argument_source, encode_offset,
    },
    push_u32, push_u64,
    unit_structural_scalar_field_store_codec::{
        decode_destination, decode_path, encode_destination, encode_path,
    },
    value_placement_codec::{decode_direct_placement, encode_direct_placement},
};

pub(super) fn encode_scalar_structural_scalar_field_store(
    bytes: &mut Vec<u8>,
    store: Option<&ScalarStructuralScalarFieldStoreRecord>,
) -> Result<(), InstallationError> {
    let Some(store) = store else {
        bytes.extend_from_slice(&[0; 4]);
        return Ok(());
    };
    bytes.extend_from_slice(&[1, 0, 0, 0]);
    push_u64(bytes, store.psi_operation.get());
    encode_destination(bytes, &store.destination)?;
    encode_path(bytes, &store.path)?;
    push_u64(bytes, store.field.get());
    encode_direct_placement(bytes, &store.destination_placement)?;
    push_u32(bytes, store.field_byte_offset);
    encode_argument_source(
        bytes,
        InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            defining_operation: store.defining_operation,
            source_value: store.source_value,
            scalar_type: store.scalar_type,
            value: store.value,
        },
    )?;
    encode_offset(bytes, store.operation_ordinal)?;
    encode_offset(bytes, store.code_offset)?;
    encode_offset(bytes, store.byte_count)?;
    push_u32(
        bytes,
        u32::try_from(store.bytes.len())
            .map_err(|_| InstallationError::TooManyUnitStructuralScalarFieldStoreBytes)?,
    );
    bytes.extend_from_slice(&store.bytes);
    Ok(())
}

pub(super) fn decode_scalar_structural_scalar_field_store(
    reader: &mut Reader<'_>,
) -> Result<Option<ScalarStructuralScalarFieldStoreRecord>, InstallationError> {
    match reader.u8()? {
        0 => {
            if reader.take(3)? != [0; 3] {
                return Err(InstallationError::NonzeroReservedField);
            }
            Ok(None)
        }
        1 => {
            if reader.take(3)? != [0; 3] {
                return Err(InstallationError::NonzeroReservedField);
            }
            let psi_operation = OperationId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroInstalledScalarIdentity)?;
            let destination = decode_destination(reader)?;
            let path = decode_path(reader)?;
            let field = StructuralFieldId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroInstalledScalarIdentity)?;
            let destination_placement = decode_direct_placement(reader)?;
            let field_byte_offset = reader.u32()?;
            let InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
                defining_operation,
                source_value,
                scalar_type,
                value,
            } = decode_argument_source(reader)?
            else {
                return Err(InstallationError::NonCanonicalEncoding);
            };
            let operation_ordinal = decode_offset(reader)?;
            let code_offset = decode_offset(reader)?;
            let byte_count = decode_offset(reader)?;
            let bytes_len = usize::try_from(reader.u32()?)
                .map_err(|_| InstallationError::TooManyUnitStructuralScalarFieldStoreBytes)?;
            let store_bytes = reader.take(bytes_len)?.to_vec();
            Ok(Some(ScalarStructuralScalarFieldStoreRecord {
                psi_operation,
                destination,
                path,
                field,
                destination_placement,
                field_byte_offset,
                defining_operation,
                source_value,
                scalar_type,
                value,
                operation_ordinal,
                code_offset,
                byte_count,
                bytes: store_bytes,
            }))
        }
        tag => Err(InstallationError::InvalidPresenceFlag(tag)),
    }
}
