//! Canonical provider-candidate declaration wire format.
//!
//! This module owns the exact identity, signature, refinement, domain, and
//! realized-service-ceiling byte order. Provider validity and selection remain
//! outside the codec.

use terminal_psi::{
    ProviderCandidateConformance, ProviderInstanceRequirements, ProviderParameterRefinement,
    ProviderRefinement, ProviderSignature, ProviderSignatureParameter, RequiredProviderInstance,
};

use super::CodecError;
use super::structural_signature_wire::DomainRequirement;
use super::wire::{Reader, Writer};
use super::wire_fields::{
    Access, Field, Id, Ids, Index, List, Multiplicity, Nominal, Optional, PathQualification,
    ProviderPackage, U32, wire_record,
};

wire_record! {
    InstanceRequirements: ProviderInstanceRequirements {
        template: Index,
        package: ProviderPackage,
        selecting_declaration: Optional<Nominal>,
        source_candidate_identity: Index,
        required_instances: List<RequiredInstance>,
    }
    RequiredInstance: RequiredProviderInstance {
        requirement_identity: Index,
        candidate_identity: Index,
        receiving_application: Index,
        receiving_region_arguments: List<Index>,
    }
    Candidate: ProviderCandidateConformance {
        boundary: Id,
        requirement_identity: Index,
        source_requirement_identity: Index,
        receiving_application: Index,
        receiving_region_arguments: List<Index>,
        provider_identity: Index,
        provider_declaration: Nominal,
        candidate_identity: Index,
        source_candidate_identity: Index,
        source_candidate_package_identity: ProviderPackage,
        candidate: Id,
        signature: Signature,
        refinement: Refinement,
    }
    Signature: ProviderSignature { parameters: List<SignatureParameter> }
    SignatureParameter: ProviderSignatureParameter {
        position: U32,
        structural_type: Id,
        multiplicity: Multiplicity,
        access: Access,
        qualifications: Ids,
        projected_qualifications: List<PathQualification>,
    }
    Refinement: ProviderRefinement {
        positional_parameters: List<ParameterRefinement>,
        required_domains: List<DomainRequirement>,
        realized_service_ceiling: Ids,
    }
    ParameterRefinement: ProviderParameterRefinement { boundary_index: U32, candidate_index: U32 }
}

pub(crate) fn encode_provider_candidate(
    writer: &mut Writer,
    candidate: &ProviderCandidateConformance,
) -> Result<(), CodecError> {
    Candidate::write(writer, candidate)
}

pub(crate) fn decode_provider_candidate(
    reader: &mut Reader<'_>,
) -> Result<ProviderCandidateConformance, CodecError> {
    Candidate::read(reader)
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
