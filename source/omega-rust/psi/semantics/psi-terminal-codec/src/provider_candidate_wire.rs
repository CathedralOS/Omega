//! Canonical provider-candidate declaration wire format.
//!
//! This module owns the exact identity, signature, refinement, domain, and
//! realized-service-ceiling byte order. Provider validity and selection remain
//! outside the codec.

use psi_terminal::{
    ProviderCandidateConformance, ProviderParameterRefinement, ProviderSignatureParameter,
    ProviderUnitRefinement, ProviderUnitSignature, StructuralDomainRequirement,
    StructuralMultiplicity,
};

use super::structural_signature_wire::{
    decode_structural_access, encode_service_ceiling, encode_structural_access,
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
        signature: ProviderUnitSignature { parameters },
        refinement: ProviderUnitRefinement {
            positional_parameters,
            required_domains,
            realized_service_ceiling: decode_ids(reader, "ServiceId")?,
        },
    })
}
