//! Canonical transport for attached-Unit structural scalar field-store rows.

use omega_machine_code::UnitStructuralScalarFieldStoreRecord;
use psi_core::{OperationId, PlaceId, StructuralDomainId, StructuralFieldId, StructuralTypeId};
use psi_terminal::{
    StructuralParameterDeclaration, StructuralPathQualification, StructuralPathSegment,
};

use super::{
    InstallationError, Reader, decode_boolean,
    internal_unit_scalar_call_codec::{
        decode_argument_source, decode_offset, encode_argument_source, encode_offset,
    },
    push_u32, push_u64,
    structural_scalar_codec::{
        access_tag, decode_access, decode_domains, decode_multiplicity, encode_domains,
        multiplicity_tag,
    },
    value_placement_codec::{decode_direct_placement, encode_direct_placement},
};

pub(super) fn encode_unit_structural_scalar_field_stores(
    bytes: &mut Vec<u8>,
    stores: &[UnitStructuralScalarFieldStoreRecord],
) -> Result<(), InstallationError> {
    push_u32(
        bytes,
        u32::try_from(stores.len())
            .map_err(|_| InstallationError::TooManyUnitStructuralScalarFieldStores)?,
    );
    for store in stores {
        push_u64(bytes, store.psi_operation.get());
        encode_destination(bytes, &store.destination)?;
        encode_path(bytes, &store.path)?;
        push_u64(bytes, store.field.get());
        encode_direct_placement(bytes, &store.destination_placement)?;
        push_u32(bytes, store.field_byte_offset);
        encode_argument_source(bytes, store.source)?;
        push_u32(bytes, store.parameter_home_byte_offset);
        bytes.push(u8::from(store.parameter_home_indirect));
        bytes.extend_from_slice(&[0; 3]);
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

pub(super) fn decode_unit_structural_scalar_field_stores(
    reader: &mut Reader<'_>,
) -> Result<Vec<UnitStructuralScalarFieldStoreRecord>, InstallationError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyUnitStructuralScalarFieldStores)?;
    if count > reader.remaining() / 96 {
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
        let source = decode_argument_source(reader)?;
        let parameter_home_byte_offset = reader.u32()?;
        let parameter_home_indirect = decode_boolean(reader.u8()?)?;
        if reader.take(3)? != [0; 3] {
            return Err(InstallationError::NonzeroReservedField);
        }
        let operation_ordinal = decode_offset(reader)?;
        let code_offset = decode_offset(reader)?;
        let byte_count = decode_offset(reader)?;
        let bytes_len = usize::try_from(reader.u32()?)
            .map_err(|_| InstallationError::TooManyUnitStructuralScalarFieldStoreBytes)?;
        let store_bytes = reader.take(bytes_len)?.to_vec();
        stores.push(UnitStructuralScalarFieldStoreRecord {
            psi_operation,
            destination,
            path,
            field,
            destination_placement,
            field_byte_offset,
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

fn encode_destination(
    bytes: &mut Vec<u8>,
    destination: &StructuralParameterDeclaration,
) -> Result<(), InstallationError> {
    push_u64(bytes, destination.place.get());
    push_u32(bytes, destination.position);
    bytes.push(u8::from(destination.is_self));
    bytes.push(multiplicity_tag(destination.multiplicity));
    bytes.push(access_tag(destination.access));
    bytes.push(0);
    push_u64(bytes, destination.structural_type.get());
    encode_domains(bytes, &destination.qualifications)?;
    encode_projected_qualifications(bytes, &destination.projected_qualifications)
}

fn decode_destination(
    reader: &mut Reader<'_>,
) -> Result<StructuralParameterDeclaration, InstallationError> {
    let place =
        PlaceId::new(reader.u64()?).ok_or(InstallationError::ZeroInstalledScalarIdentity)?;
    let position = reader.u32()?;
    let is_self = decode_boolean(reader.u8()?)?;
    let multiplicity = decode_multiplicity(reader.u8()?)?;
    let access = decode_access(reader.u8()?)?;
    if reader.u8()? != 0 {
        return Err(InstallationError::NonzeroReservedField);
    }
    let structural_type = StructuralTypeId::new(reader.u64()?)
        .ok_or(InstallationError::ZeroInstalledScalarIdentity)?;
    Ok(StructuralParameterDeclaration {
        place,
        position,
        is_self,
        structural_type,
        multiplicity,
        access,
        qualifications: decode_domains(reader)?,
        projected_qualifications: decode_projected_qualifications(reader)?,
    })
}

fn encode_path(
    bytes: &mut Vec<u8>,
    path: &[StructuralPathSegment],
) -> Result<(), InstallationError> {
    push_u32(
        bytes,
        u32::try_from(path.len())
            .map_err(|_| InstallationError::TooManySettlementArgumentPathSegments)?,
    );
    for segment in path {
        match segment {
            StructuralPathSegment::Field(identity) => {
                if identity.is_empty() {
                    return Err(InstallationError::InvalidSettlementArgumentField);
                }
                bytes.extend_from_slice(&[1, 0, 0, 0]);
                push_u32(
                    bytes,
                    u32::try_from(identity.len())
                        .map_err(|_| InstallationError::SettlementArgumentFieldTooLong)?,
                );
                bytes.extend_from_slice(identity.as_bytes());
            }
            StructuralPathSegment::FixedIndex(index) => {
                bytes.extend_from_slice(&[2, 0, 0, 0]);
                push_u64(bytes, *index);
            }
        }
    }
    Ok(())
}

fn decode_path(reader: &mut Reader<'_>) -> Result<Vec<StructuralPathSegment>, InstallationError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManySettlementArgumentPathSegments)?;
    if count > reader.remaining() / 8 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut path = Vec::with_capacity(count);
    for _ in 0..count {
        let tag = reader.u8()?;
        if reader.take(3)? != [0; 3] {
            return Err(InstallationError::NonzeroReservedField);
        }
        path.push(match tag {
            1 => {
                let len = usize::try_from(reader.u32()?)
                    .map_err(|_| InstallationError::SettlementArgumentFieldTooLong)?;
                let identity = std::str::from_utf8(reader.take(len)?)
                    .map_err(|_| InstallationError::InvalidSettlementArgumentField)?
                    .to_owned();
                if identity.is_empty() {
                    return Err(InstallationError::InvalidSettlementArgumentField);
                }
                StructuralPathSegment::Field(identity)
            }
            2 => StructuralPathSegment::FixedIndex(reader.u64()?),
            tag => return Err(InstallationError::InvalidSettlementArgumentPathTag(tag)),
        });
    }
    Ok(path)
}

fn encode_projected_qualifications(
    bytes: &mut Vec<u8>,
    qualifications: &[StructuralPathQualification],
) -> Result<(), InstallationError> {
    push_u32(
        bytes,
        u32::try_from(qualifications.len())
            .map_err(|_| InstallationError::TooManyStructuralQualifications)?,
    );
    for qualification in qualifications {
        encode_path(bytes, &qualification.path)?;
        push_u64(bytes, qualification.domain.get());
    }
    Ok(())
}

fn decode_projected_qualifications(
    reader: &mut Reader<'_>,
) -> Result<Vec<StructuralPathQualification>, InstallationError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyStructuralQualifications)?;
    let mut qualifications = Vec::with_capacity(count);
    for _ in 0..count {
        qualifications.push(StructuralPathQualification {
            path: decode_path(reader)?,
            domain: StructuralDomainId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroInstalledScalarIdentity)?,
        });
    }
    Ok(qualifications)
}
