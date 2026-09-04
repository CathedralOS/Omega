//! Canonical format-37 codec for installed function parameters and homes.
//!
//! Unit/scalar row positions remain in the installation parent. This child
//! shares their exact bytes while retaining the established decode labels.

use omega_machine_code::{UnitParameterHomeRecord, UnitParameterRecord};
use psi_core::{PlaceId, StructuralTypeId};

use super::structural_scalar_codec::{access_tag, decode_access};
use super::{
    InstallationError, Reader, decode_boolean, decode_multiplicity, multiplicity_tag, push_u32,
    push_u64,
    value_placement_codec::{
        decode_direct_placement, decode_shape, encode_direct_placement, encode_shape,
    },
};

pub(super) fn encode_parameter_records(
    bytes: &mut Vec<u8>,
    parameters: &[UnitParameterRecord],
) -> Result<(), InstallationError> {
    push_u32(
        bytes,
        u32::try_from(parameters.len())
            .map_err(|_| InstallationError::TooManyStructuralReturnParameters)?,
    );
    for parameter in parameters {
        push_u64(bytes, parameter.place.get());
        push_u64(bytes, parameter.structural_type.get());
        bytes.push(multiplicity_tag(parameter.multiplicity));
        bytes.push(access_tag(parameter.access));
        bytes.extend_from_slice(&[0; 2]);
        encode_shape(bytes, parameter.shape)?;
    }
    Ok(())
}

pub(super) fn encode_parameter_homes(
    bytes: &mut Vec<u8>,
    homes: &[UnitParameterHomeRecord],
) -> Result<(), InstallationError> {
    push_u32(
        bytes,
        u32::try_from(homes.len())
            .map_err(|_| InstallationError::TooManyStructuralReturnParameters)?,
    );
    for home in homes {
        push_u64(bytes, home.place.get());
        push_u64(bytes, home.structural_type.get());
        bytes.push(multiplicity_tag(home.multiplicity));
        bytes.push(access_tag(home.access));
        bytes.extend_from_slice(&[0; 2]);
        encode_shape(bytes, home.shape)?;
        encode_direct_placement(bytes, &home.source)?;
        push_u32(bytes, home.byte_offset);
        bytes.push(u8::from(home.indirect));
        bytes.extend_from_slice(&[0; 3]);
    }
    Ok(())
}

pub(super) fn decode_unit_parameter_records(
    reader: &mut Reader<'_>,
) -> Result<Vec<UnitParameterRecord>, InstallationError> {
    decode_parameter_records(reader, "Unit parameter place", "Unit parameter type")
}

pub(super) fn decode_unit_parameter_homes(
    reader: &mut Reader<'_>,
) -> Result<Vec<UnitParameterHomeRecord>, InstallationError> {
    decode_parameter_homes(reader, "Unit home place", "Unit home type")
}

pub(super) fn decode_scalar_parameter_records(
    reader: &mut Reader<'_>,
) -> Result<Vec<UnitParameterRecord>, InstallationError> {
    decode_parameter_records(reader, "scalar parameter place", "scalar parameter type")
}

pub(super) fn decode_scalar_parameter_homes(
    reader: &mut Reader<'_>,
) -> Result<Vec<UnitParameterHomeRecord>, InstallationError> {
    decode_parameter_homes(reader, "scalar home place", "scalar home type")
}

fn decode_parameter_records(
    reader: &mut Reader<'_>,
    place_identity: &'static str,
    type_identity: &'static str,
) -> Result<Vec<UnitParameterRecord>, InstallationError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyStructuralReturnParameters)?;
    let mut parameters = Vec::with_capacity(count);
    for _ in 0..count {
        let place = PlaceId::new(reader.u64()?).ok_or(
            InstallationError::ZeroStructuralReturnIdentity(place_identity),
        )?;
        let structural_type = StructuralTypeId::new(reader.u64()?).ok_or(
            InstallationError::ZeroStructuralReturnIdentity(type_identity),
        )?;
        let multiplicity = decode_multiplicity(reader.u8()?)?;
        let access = decode_access(reader.u8()?)?;
        if reader.take(2)? != [0; 2] {
            return Err(InstallationError::NonzeroReservedField);
        }
        parameters.push(UnitParameterRecord {
            place,
            structural_type,
            multiplicity,
            access,
            shape: decode_shape(reader)?,
        });
    }
    Ok(parameters)
}

fn decode_parameter_homes(
    reader: &mut Reader<'_>,
    place_identity: &'static str,
    type_identity: &'static str,
) -> Result<Vec<UnitParameterHomeRecord>, InstallationError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyStructuralReturnParameters)?;
    let mut homes = Vec::with_capacity(count);
    for _ in 0..count {
        let place = PlaceId::new(reader.u64()?).ok_or(
            InstallationError::ZeroStructuralReturnIdentity(place_identity),
        )?;
        let structural_type = StructuralTypeId::new(reader.u64()?).ok_or(
            InstallationError::ZeroStructuralReturnIdentity(type_identity),
        )?;
        let multiplicity = decode_multiplicity(reader.u8()?)?;
        let access = decode_access(reader.u8()?)?;
        if reader.take(2)? != [0; 2] {
            return Err(InstallationError::NonzeroReservedField);
        }
        let shape = decode_shape(reader)?;
        let source = decode_direct_placement(reader)?;
        let byte_offset = reader.u32()?;
        let indirect = decode_boolean(reader.u8()?)?;
        if reader.take(3)? != [0; 3] {
            return Err(InstallationError::NonzeroReservedField);
        }
        homes.push(UnitParameterHomeRecord {
            place,
            structural_type,
            multiplicity,
            access,
            shape,
            source,
            byte_offset,
            indirect,
        });
    }
    Ok(homes)
}
