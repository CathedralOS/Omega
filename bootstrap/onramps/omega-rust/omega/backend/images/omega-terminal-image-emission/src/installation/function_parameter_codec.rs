//! Canonical format-36 codec for installed function parameters and homes.
//!
//! Unit/scalar row positions remain in the installation parent. This child
//! shares their exact bytes while retaining the established decode labels.

use omega_terminal_machine_code::{TerminalUnitParameterHomeRecord, TerminalUnitParameterRecord};
use psi_core::{PlaceId, StructuralTypeId};

use super::{
    Reader, TerminalInstallationError, decode_boolean, decode_multiplicity, multiplicity_tag,
    push_u32, push_u64,
    value_placement_codec::{
        decode_direct_placement, decode_shape, encode_direct_placement, encode_shape,
    },
};

pub(super) fn encode_parameter_records(
    bytes: &mut Vec<u8>,
    parameters: &[TerminalUnitParameterRecord],
) -> Result<(), TerminalInstallationError> {
    push_u32(
        bytes,
        u32::try_from(parameters.len())
            .map_err(|_| TerminalInstallationError::TooManyStructuralReturnParameters)?,
    );
    for parameter in parameters {
        push_u64(bytes, parameter.place.get());
        push_u64(bytes, parameter.structural_type.get());
        bytes.push(multiplicity_tag(parameter.multiplicity));
        bytes.extend_from_slice(&[0; 3]);
        encode_shape(bytes, parameter.shape)?;
    }
    Ok(())
}

pub(super) fn encode_parameter_homes(
    bytes: &mut Vec<u8>,
    homes: &[TerminalUnitParameterHomeRecord],
) -> Result<(), TerminalInstallationError> {
    push_u32(
        bytes,
        u32::try_from(homes.len())
            .map_err(|_| TerminalInstallationError::TooManyStructuralReturnParameters)?,
    );
    for home in homes {
        push_u64(bytes, home.place.get());
        push_u64(bytes, home.structural_type.get());
        bytes.push(multiplicity_tag(home.multiplicity));
        bytes.extend_from_slice(&[0; 3]);
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
) -> Result<Vec<TerminalUnitParameterRecord>, TerminalInstallationError> {
    decode_parameter_records(reader, "Unit parameter place", "Unit parameter type")
}

pub(super) fn decode_unit_parameter_homes(
    reader: &mut Reader<'_>,
) -> Result<Vec<TerminalUnitParameterHomeRecord>, TerminalInstallationError> {
    decode_parameter_homes(reader, "Unit home place", "Unit home type")
}

pub(super) fn decode_scalar_parameter_records(
    reader: &mut Reader<'_>,
) -> Result<Vec<TerminalUnitParameterRecord>, TerminalInstallationError> {
    decode_parameter_records(reader, "scalar parameter place", "scalar parameter type")
}

pub(super) fn decode_scalar_parameter_homes(
    reader: &mut Reader<'_>,
) -> Result<Vec<TerminalUnitParameterHomeRecord>, TerminalInstallationError> {
    decode_parameter_homes(reader, "scalar home place", "scalar home type")
}

fn decode_parameter_records(
    reader: &mut Reader<'_>,
    place_identity: &'static str,
    type_identity: &'static str,
) -> Result<Vec<TerminalUnitParameterRecord>, TerminalInstallationError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyStructuralReturnParameters)?;
    let mut parameters = Vec::with_capacity(count);
    for _ in 0..count {
        let place = PlaceId::new(reader.u64()?).ok_or(
            TerminalInstallationError::ZeroStructuralReturnIdentity(place_identity),
        )?;
        let structural_type = StructuralTypeId::new(reader.u64()?).ok_or(
            TerminalInstallationError::ZeroStructuralReturnIdentity(type_identity),
        )?;
        let multiplicity = decode_multiplicity(reader.u8()?)?;
        if reader.take(3)? != [0; 3] {
            return Err(TerminalInstallationError::NonzeroReservedField);
        }
        parameters.push(TerminalUnitParameterRecord {
            place,
            structural_type,
            multiplicity,
            shape: decode_shape(reader)?,
        });
    }
    Ok(parameters)
}

fn decode_parameter_homes(
    reader: &mut Reader<'_>,
    place_identity: &'static str,
    type_identity: &'static str,
) -> Result<Vec<TerminalUnitParameterHomeRecord>, TerminalInstallationError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyStructuralReturnParameters)?;
    let mut homes = Vec::with_capacity(count);
    for _ in 0..count {
        let place = PlaceId::new(reader.u64()?).ok_or(
            TerminalInstallationError::ZeroStructuralReturnIdentity(place_identity),
        )?;
        let structural_type = StructuralTypeId::new(reader.u64()?).ok_or(
            TerminalInstallationError::ZeroStructuralReturnIdentity(type_identity),
        )?;
        let multiplicity = decode_multiplicity(reader.u8()?)?;
        if reader.take(3)? != [0; 3] {
            return Err(TerminalInstallationError::NonzeroReservedField);
        }
        let shape = decode_shape(reader)?;
        let source = decode_direct_placement(reader)?;
        let byte_offset = reader.u32()?;
        let indirect = decode_boolean(reader.u8()?)?;
        if reader.take(3)? != [0; 3] {
            return Err(TerminalInstallationError::NonzeroReservedField);
        }
        homes.push(TerminalUnitParameterHomeRecord {
            place,
            structural_type,
            multiplicity,
            shape,
            source,
            byte_offset,
            indirect,
        });
    }
    Ok(homes)
}
