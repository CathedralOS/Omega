//! Canonical provider-candidate declaration wire format.
//!
//! This module owns the exact identity, signature, refinement, domain, and
//! realized-service-ceiling byte order. Provider validity and selection remain
//! outside the codec.

use terminal_psi::{
    ProviderCandidateConformance, ProviderParameterRefinement, ProviderRefinement,
    ProviderSignature, ProviderSignatureParameter, StructuralDomainRequirement,
    StructuralMultiplicity,
};

use super::structural_signature_wire::{
    decode_projected_qualifications, decode_structural_access, encode_projected_qualifications,
    encode_service_ceiling, encode_structural_access,
};
use super::wire::{Reader, Writer};
use super::{CodecError, decode_counted, decode_ids};

pub(super) fn encode_provider_candidate(
    writer: &mut Writer,
    candidate: &ProviderCandidateConformance,
) -> Result<(), CodecError> {
    writer.id(candidate.boundary);
    writer.string(
        "provider requirement identity",
        &candidate.requirement_identity,
    )?;
    writer.string("provider identity", &candidate.provider_identity)?;
    writer.string("provider candidate identity", &candidate.candidate_identity)?;
    writer.id(candidate.candidate);
    writer.len(
        "provider signature parameters",
        candidate.signature.parameters.len(),
    )?;
    for parameter in &candidate.signature.parameters {
        writer.u32(parameter.position);
        writer.u8(u8::from(parameter.is_self));
        writer.id(parameter.structural_type);
        writer.u8(match parameter.multiplicity {
            StructuralMultiplicity::Unrestricted => 1,
            StructuralMultiplicity::Affine => 2,
            StructuralMultiplicity::Linear => 3,
        });
        encode_structural_access(writer, parameter.access);
        writer.len(
            "provider signature qualifications",
            parameter.qualifications.len(),
        )?;
        for qualification in &parameter.qualifications {
            writer.id(*qualification);
        }
        encode_projected_qualifications(writer, &parameter.projected_qualifications)?;
    }
    writer.len(
        "provider positional refinements",
        candidate.refinement.positional_parameters.len(),
    )?;
    for parameter in &candidate.refinement.positional_parameters {
        writer.u32(parameter.boundary_index);
        writer.u32(parameter.candidate_index);
    }
    writer.len(
        "provider required domains",
        candidate.refinement.required_domains.len(),
    )?;
    for requirement in &candidate.refinement.required_domains {
        writer.u32(requirement.argument_index);
        writer.id(requirement.domain);
    }
    encode_service_ceiling(writer, &candidate.refinement.realized_service_ceiling)
}

pub(super) fn decode_provider_candidate(
    reader: &mut Reader<'_>,
) -> Result<ProviderCandidateConformance, CodecError> {
    let boundary = reader.id("BoundaryMachineId")?;
    let requirement_identity = reader.string("provider requirement identity")?;
    let provider_identity = reader.string("provider identity")?;
    let candidate_identity = reader.string("provider candidate identity")?;
    let candidate = reader.id("MachineId")?;
    let parameters = decode_counted(reader, |reader| {
        let position = reader.u32()?;
        let is_self = reader.boolean()?;
        let structural_type = reader.id("StructuralTypeId")?;
        let multiplicity = match reader.u8()? {
            1 => StructuralMultiplicity::Unrestricted,
            2 => StructuralMultiplicity::Affine,
            3 => StructuralMultiplicity::Linear,
            tag => return Err(CodecError::InvalidTag("StructuralMultiplicity", tag)),
        };
        Ok(ProviderSignatureParameter {
            position,
            is_self,
            structural_type,
            multiplicity,
            access: decode_structural_access(reader)?,
            qualifications: decode_ids(reader, "StructuralDomainId")?,
            projected_qualifications: decode_projected_qualifications(reader)?,
        })
    })?;
    let positional_parameters = decode_counted(reader, |reader| {
        Ok(ProviderParameterRefinement {
            boundary_index: reader.u32()?,
            candidate_index: reader.u32()?,
        })
    })?;
    let required_domains = decode_counted(reader, |reader| {
        Ok(StructuralDomainRequirement {
            argument_index: reader.u32()?,
            domain: reader.id("StructuralDomainId")?,
        })
    })?;
    Ok(ProviderCandidateConformance {
        boundary,
        requirement_identity,
        provider_identity,
        candidate_identity,
        candidate,
        signature: ProviderSignature { parameters },
        refinement: ProviderRefinement {
            positional_parameters,
            required_domains,
            realized_service_ceiling: decode_ids(reader, "ServiceId")?,
        },
    })
}

/// Encode one provider declaration using the current Terminal vocabulary.
/// This preserves data only; it does not validate conformance or grant authority.
pub fn encode_provider_candidate_record(
    candidate: &ProviderCandidateConformance,
) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    writer.u16(super::FORMAT_MARKER);
    writer.u16(terminal_psi::VocabularyMarker::CURRENT.get());
    encode_provider_candidate(&mut writer, candidate)?;
    Ok(writer.finish())
}

/// Decode one complete provider declaration, rejecting other vocabulary versions
/// and trailing bytes. Semantic conformance remains the verifier's responsibility.
pub fn decode_provider_candidate_record(
    bytes: &[u8],
) -> Result<ProviderCandidateConformance, CodecError> {
    let mut reader = Reader::new(bytes);
    let format = reader.u16()?;
    if format != super::FORMAT_MARKER {
        return Err(CodecError::UnsupportedFormatMarker(format));
    }
    let marker = reader.u16()?;
    if marker != terminal_psi::VocabularyMarker::CURRENT.get() {
        return Err(CodecError::UnsupportedVocabularyMarker(marker));
    }
    let candidate = decode_provider_candidate(&mut reader)?;
    if reader.remaining() != 0 {
        return Err(CodecError::TrailingBytes(reader.remaining()));
    }
    Ok(candidate)
}
#[cfg(test)]
mod record_tests {
    use super::*;
    fn candidate() -> ProviderCandidateConformance {
        ProviderCandidateConformance {
            boundary: semantic_vocabulary::BoundaryMachineId::new(1).unwrap(),
            requirement_identity: "requirement".into(),
            provider_identity: "provider".into(),
            candidate_identity: "candidate".into(),
            candidate: semantic_vocabulary::MachineId::new(2).unwrap(),
            signature: ProviderSignature {
                parameters: Vec::new(),
            },
            refinement: ProviderRefinement {
                positional_parameters: Vec::new(),
                required_domains: Vec::new(),
                realized_service_ceiling: Vec::new(),
            },
        }
    }
    #[test]
    fn provider_record_round_trip_retains_current_terminal_wire_contract() {
        let candidate = candidate();
        let bytes = encode_provider_candidate_record(&candidate).unwrap();
        assert_eq!(&bytes[..2], &super::super::FORMAT_MARKER.to_le_bytes());
        assert_eq!(
            &bytes[2..4],
            &terminal_psi::VocabularyMarker::CURRENT.get().to_le_bytes()
        );
        assert_eq!(decode_provider_candidate_record(&bytes).unwrap(), candidate);
    }
    #[test]
    fn provider_record_rejects_trailing_bytes_and_stale_markers() {
        let bytes = encode_provider_candidate_record(&candidate()).unwrap();
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert_eq!(
            decode_provider_candidate_record(&trailing),
            Err(CodecError::TrailingBytes(1))
        );
        let mut format = bytes.clone();
        format[..2].copy_from_slice(&0u16.to_le_bytes());
        assert_eq!(
            decode_provider_candidate_record(&format),
            Err(CodecError::UnsupportedFormatMarker(0))
        );
        let mut vocabulary = bytes;
        vocabulary[2..4].copy_from_slice(&0u16.to_le_bytes());
        assert_eq!(
            decode_provider_candidate_record(&vocabulary),
            Err(CodecError::UnsupportedVocabularyMarker(0))
        );
    }
}
