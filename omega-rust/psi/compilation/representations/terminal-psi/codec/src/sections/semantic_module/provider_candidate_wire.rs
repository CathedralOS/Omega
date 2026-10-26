//! Canonical provider-candidate declaration wire format.
//!
//! This module owns the exact identity, signature, refinement, domain, and
//! realized-service-ceiling byte order. Provider validity and selection remain
//! outside the codec.

use terminal_psi::{
    ProviderCandidateConformance, ProviderParameterRefinement, ProviderRefinement,
    ProviderSignature, ProviderSignatureParameter, StructuralDomainRequirement,
};

use super::CodecError;
use super::structural_result_wire::{decode_multiplicity, encode_multiplicity};
use super::structural_signature_wire::{
    decode_projected_qualifications, decode_structural_access, encode_projected_qualifications,
    encode_service_ceiling, encode_structural_access,
};
use super::wire::{Reader, Writer};
use crate::sections::semantic_module::wire::{decode_counted, decode_ids};
use terminal_psi::TerminalIdentityIndex;

pub(crate) fn encode_provider_candidate(
    writer: &mut Writer,
    candidate: &ProviderCandidateConformance,
) -> Result<(), CodecError> {
    writer.id(candidate.boundary);
    writer.u32(candidate.requirement_identity.0);
    writer.u32(candidate.source_requirement_identity.0);
    writer.u32(candidate.receiving_application.0);
    writer.len(
        "provider receiving region arguments",
        candidate.receiving_region_arguments.len(),
    )?;
    for region in &candidate.receiving_region_arguments {
        writer.u32(region.0);
    }
    writer.u32(candidate.provider_identity.0);
    writer.u32(candidate.provider_declaration.identity().0);
    writer.u32(candidate.candidate_identity.0);
    writer.u32(candidate.source_candidate_identity.0);
    writer.boolean(candidate.source_candidate_package_identity.is_some());
    if let Some(package) = candidate.source_candidate_package_identity {
        writer.bytes(&package.digest());
    }
    writer.id(candidate.candidate);
    writer.len(
        "provider signature parameters",
        candidate.signature.parameters.len(),
    )?;
    for parameter in &candidate.signature.parameters {
        writer.u32(parameter.position);
        writer.id(parameter.structural_type);
        encode_multiplicity(writer, parameter.multiplicity);
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

pub(crate) fn decode_provider_candidate(
    reader: &mut Reader<'_>,
) -> Result<ProviderCandidateConformance, CodecError> {
    let boundary = reader.id("BoundaryMachineId")?;
    let requirement_identity = TerminalIdentityIndex(reader.u32()?);
    let source_requirement_identity = TerminalIdentityIndex(reader.u32()?);
    let receiving_application = TerminalIdentityIndex(reader.u32()?);
    let receiving_region_arguments =
        decode_counted(reader, |reader| Ok(TerminalIdentityIndex(reader.u32()?)))?;
    let provider_identity = TerminalIdentityIndex(reader.u32()?);
    let provider_declaration =
        terminal_psi::NominalDeclarationIdentity::new(TerminalIdentityIndex(reader.u32()?));
    let candidate_identity = TerminalIdentityIndex(reader.u32()?);
    let source_candidate_identity = TerminalIdentityIndex(reader.u32()?);
    let source_candidate_package_identity = if reader.boolean()? {
        Some(
            semantic_vocabulary::PackageKeyIdentity::from_digest(reader.array()?).ok_or(
                CodecError::MalformedStructuralFoundation("empty provider source package identity"),
            )?,
        )
    } else {
        None
    };
    let candidate = reader.id("MachineId")?;
    let parameters = decode_counted(reader, |reader| {
        let position = reader.u32()?;
        let structural_type = reader.id("StructuralTypeId")?;
        let multiplicity = decode_multiplicity(reader)?;
        Ok(ProviderSignatureParameter {
            position,
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
        source_requirement_identity,
        receiving_application,
        receiving_region_arguments,
        provider_identity,
        provider_declaration,
        candidate_identity,
        source_candidate_identity,
        source_candidate_package_identity,
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
