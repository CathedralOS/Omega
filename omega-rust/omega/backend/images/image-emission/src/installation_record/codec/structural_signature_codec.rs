//! Canonical format-36 structural source and result declaration rows.

use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::{StructuralParameterDeclaration, StructuralResultDeclaration};

use super::structural_argument_codec::{
    decode_path, decode_structural_argument, encode_path, encode_structural_argument,
};
use super::structural_scalar_codec::{
    access_tag, decode_access, decode_domains, decode_multiplicity, encode_domains,
    multiplicity_tag,
};
use crate::installation_record::{InstallationError, Reader, decode_boolean, push_u32, push_u64};

pub(crate) fn encode_structural_parameter(
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

/// Reference-bearing results preserve their declared source roster on the
/// wire: each row binds a result-carrier path to the exact callee argument
/// whose referent the caller inherits. Erasing the roster would describe an
/// apparently ordinary owned result.
pub(crate) fn encode_structural_result(
    bytes: &mut Vec<u8>,
    result: &StructuralResultDeclaration,
) -> Result<(), InstallationError> {
    push_u64(bytes, result.place.get());
    push_u64(bytes, result.structural_type.get());
    bytes.push(multiplicity_tag(result.multiplicity));
    bytes.extend_from_slice(&[0; 3]);
    encode_domains(bytes, &result.qualifications)?;
    push_u32(
        bytes,
        u32::try_from(result.reference_sources.len())
            .map_err(|_| InstallationError::TooManyStructuralReturnReferenceSources)?,
    );
    for source in &result.reference_sources {
        encode_path(bytes, &source.path)?;
        encode_structural_argument(bytes, &source.source)?;
    }
    Ok(())
}

pub(crate) fn decode_structural_parameter(
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

pub(crate) fn decode_structural_result(
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
    let qualifications = decode_domains(reader)?;
    let source_count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyStructuralReturnReferenceSources)?;
    if source_count > reader.remaining() / 4 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut reference_sources = Vec::with_capacity(source_count);
    for _ in 0..source_count {
        let path = decode_path(reader)?;
        let source = decode_structural_argument(reader)?;
        reference_sources.push(terminal_psi::StructuralReferenceResultSource { path, source });
    }
    Ok(StructuralResultDeclaration {
        reference_sources,
        place,
        structural_type,
        multiplicity,
        qualifications,
        projected_qualifications: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        PlaceId, Reader, StructuralResultDeclaration, StructuralTypeId, decode_structural_result,
        encode_structural_result,
    };
    use terminal_psi::{
        StructuralAccess, StructuralArgument, StructuralMultiplicity,
        StructuralReferenceResultSource,
    };

    #[test]
    fn native_result_wire_preserves_owned_identity_and_reference_sources() {
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
                path: vec![terminal_psi::StructuralPathSegment::Field("held".into())],
                source: StructuralArgument {
                    place: PlaceId::new(1).unwrap(),
                    path: vec![
                        terminal_psi::StructuralPathSegment::Field("body".into()),
                        terminal_psi::StructuralPathSegment::Referent,
                    ],
                    access: StructuralAccess::MutableBorrow,
                },
            });
        let mut bytes = Vec::new();
        encode_structural_result(&mut bytes, &result).unwrap();
        let mut reader = Reader::new(&bytes);
        assert_eq!(decode_structural_result(&mut reader).unwrap(), result);
        assert_eq!(reader.remaining(), 0);
        let mut reencoded = Vec::new();
        encode_structural_result(&mut reencoded, &result).unwrap();
        assert_eq!(bytes, reencoded);
    }
}
