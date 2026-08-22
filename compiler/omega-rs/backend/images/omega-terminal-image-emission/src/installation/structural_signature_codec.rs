//! Canonical format-33 structural source and result declaration rows.

use psi_core::{PlaceId, StructuralTypeId};
use psi_terminal::{StructuralParameterDeclaration, StructuralResultDeclaration};

use super::{
    Reader, TerminalInstallationError, decode_boolean, push_u32, push_u64,
    structural_scalar_codec::{
        decode_domains, decode_multiplicity, encode_domains, multiplicity_tag,
    },
};

pub(super) fn encode_structural_parameter(
    bytes: &mut Vec<u8>,
    parameter: &StructuralParameterDeclaration,
) -> Result<(), TerminalInstallationError> {
    push_u64(bytes, parameter.place.get());
    push_u32(bytes, parameter.position);
    bytes.push(u8::from(parameter.is_self));
    bytes.push(multiplicity_tag(parameter.multiplicity));
    bytes.extend_from_slice(&[0; 2]);
    push_u64(bytes, parameter.structural_type.get());
    encode_domains(bytes, &parameter.qualifications)
}

pub(super) fn encode_structural_result(
    bytes: &mut Vec<u8>,
    result: &StructuralResultDeclaration,
) -> Result<(), TerminalInstallationError> {
    push_u64(bytes, result.place.get());
    push_u64(bytes, result.structural_type.get());
    bytes.push(multiplicity_tag(result.multiplicity));
    bytes.extend_from_slice(&[0; 3]);
    encode_domains(bytes, &result.qualifications)
}

pub(super) fn decode_structural_parameter(
    reader: &mut Reader<'_>,
) -> Result<StructuralParameterDeclaration, TerminalInstallationError> {
    let place = PlaceId::new(reader.u64()?).ok_or(
        TerminalInstallationError::ZeroStructuralReturnIdentity("source place"),
    )?;
    let position = reader.u32()?;
    let is_self = decode_boolean(reader.u8()?)?;
    let multiplicity = decode_multiplicity(reader.u8()?)?;
    if reader.u16()? != 0 {
        return Err(TerminalInstallationError::NonzeroReservedField);
    }
    let structural_type = StructuralTypeId::new(reader.u64()?).ok_or(
        TerminalInstallationError::ZeroStructuralReturnIdentity("source type"),
    )?;
    Ok(StructuralParameterDeclaration {
        place,
        position,
        is_self,
        structural_type,
        multiplicity,
        qualifications: decode_domains(reader)?,
    })
}

pub(super) fn decode_structural_result(
    reader: &mut Reader<'_>,
) -> Result<StructuralResultDeclaration, TerminalInstallationError> {
    let place = PlaceId::new(reader.u64()?).ok_or(
        TerminalInstallationError::ZeroStructuralReturnIdentity("result place"),
    )?;
    let structural_type = StructuralTypeId::new(reader.u64()?).ok_or(
        TerminalInstallationError::ZeroStructuralReturnIdentity("result type"),
    )?;
    let multiplicity = decode_multiplicity(reader.u8()?)?;
    if reader.take(3)? != [0; 3] {
        return Err(TerminalInstallationError::NonzeroReservedField);
    }
    Ok(StructuralResultDeclaration {
        place,
        structural_type,
        multiplicity,
        qualifications: decode_domains(reader)?,
    })
}
