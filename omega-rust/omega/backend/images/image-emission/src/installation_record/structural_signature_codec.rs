//! Canonical format-36 structural source and result declaration rows.

use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::{StructuralParameterDeclaration, StructuralResultDeclaration};

use super::{
    InstallationError, Reader, decode_boolean, push_u32, push_u64,
    structural_scalar_codec::{
        access_tag, decode_access, decode_domains, decode_multiplicity, encode_domains,
        multiplicity_tag,
    },
};

pub(super) fn encode_structural_parameter(
    bytes: &mut Vec<u8>,
    parameter: &StructuralParameterDeclaration,
) -> Result<(), InstallationError> {
    push_u64(bytes, parameter.place.get());
    push_u32(bytes, parameter.position);
    bytes.push(u8::from(parameter.is_self));
    bytes.push(multiplicity_tag(parameter.multiplicity));
    bytes.push(access_tag(parameter.access));
    bytes.push(0);
    push_u64(bytes, parameter.structural_type.get());
    encode_domains(bytes, &parameter.qualifications)
}

pub(super) fn encode_structural_result(
    bytes: &mut Vec<u8>,
    result: &StructuralResultDeclaration,
) -> Result<(), InstallationError> {
    if !result.reference_sources.is_empty() {
        // This native format has no reference custody rows. Do not erase
        // source correspondence into an apparently ordinary owned result.
        return Err(InstallationError::UnsupportedStructuralReturnShape);
    }
    push_u64(bytes, result.place.get());
    push_u64(bytes, result.structural_type.get());
    bytes.push(multiplicity_tag(result.multiplicity));
    bytes.extend_from_slice(&[0; 3]);
    encode_domains(bytes, &result.qualifications)
}

pub(super) fn decode_structural_parameter(
    reader: &mut Reader<'_>,
) -> Result<StructuralParameterDeclaration, InstallationError> {
    let place = PlaceId::new(reader.u64()?).ok_or(
        InstallationError::ZeroStructuralReturnIdentity("source place"),
    )?;
    let position = reader.u32()?;
    let is_self = decode_boolean(reader.u8()?)?;
    let multiplicity = decode_multiplicity(reader.u8()?)?;
    let access = decode_access(reader.u8()?)?;
    if reader.u8()? != 0 {
        return Err(InstallationError::NonzeroReservedField);
    }
    let structural_type = StructuralTypeId::new(reader.u64()?).ok_or(
        InstallationError::ZeroStructuralReturnIdentity("source type"),
    )?;
    Ok(StructuralParameterDeclaration {
        place,
        position,
        is_self,
        structural_type,
        multiplicity,
        access,
        qualifications: decode_domains(reader)?,
        projected_qualifications: Vec::new(),
    })
}

pub(super) fn decode_structural_result(
    reader: &mut Reader<'_>,
) -> Result<StructuralResultDeclaration, InstallationError> {
    let place = PlaceId::new(reader.u64()?).ok_or(
        InstallationError::ZeroStructuralReturnIdentity("result place"),
    )?;
    let structural_type = StructuralTypeId::new(reader.u64()?).ok_or(
        InstallationError::ZeroStructuralReturnIdentity("result type"),
    )?;
    let multiplicity = decode_multiplicity(reader.u8()?)?;
    if reader.take(3)? != [0; 3] {
        return Err(InstallationError::NonzeroReservedField);
    }
    Ok(StructuralResultDeclaration {
        reference_sources: Vec::new(),
        place,
        structural_type,
        multiplicity,
        qualifications: decode_domains(reader)?,
        projected_qualifications: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_psi::{
        StructuralAccess, StructuralArgument, StructuralMultiplicity,
        StructuralReferenceResultSource,
    };

    #[test]
    fn native_result_wire_preserves_owned_identity_and_rejects_reference_sources() {
        let mut result = StructuralResultDeclaration {
            place: PlaceId::new(3).unwrap(),
            structural_type: StructuralTypeId::new(7).unwrap(),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            reference_sources: Vec::new(),
        };
        let mut bytes = Vec::new();
        encode_structural_result(&mut bytes, &result).unwrap();
        let mut reader = Reader::new(&bytes);
        assert_eq!(decode_structural_result(&mut reader).unwrap(), result);
        assert_eq!(reader.remaining(), 0);
        let mut reencoded = Vec::new();
        encode_structural_result(&mut reencoded, &result).unwrap();
        assert_eq!(bytes, reencoded);

        result
            .reference_sources
            .push(StructuralReferenceResultSource {
                path: Vec::new(),
                source: StructuralArgument {
                    place: PlaceId::new(1).unwrap(),
                    path: Vec::new(),
                    access: StructuralAccess::MutableBorrow,
                },
            });
        let mut rejected = Vec::new();
        assert_eq!(
            encode_structural_result(&mut rejected, &result),
            Err(InstallationError::UnsupportedStructuralReturnShape)
        );
        assert!(rejected.is_empty());
    }
}
