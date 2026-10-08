//! Installed-provider signature and refinement wire custody.

use semantic_vocabulary::{BoundaryMachineId, MachineId, ServiceId, StructuralTypeId};
use terminal_psi::{
    ProviderCandidateConformance, ProviderParameterRefinement, ProviderRefinement,
    ProviderSignature, ProviderSignatureParameter, StructuralDomainRequirement,
};

use crate::FixedViewCopyDecodeError;

use super::{
    declarations::{
        decode_access, decode_multiplicity, decode_string, encode_access, encode_multiplicity,
        encode_string,
    },
    projected_qualifications::{decode_projected, encode_projected},
};
use crate::register_homes::recovery::fixed_view_copy::codec::primitives::{
    Cursor, decode_id, decode_ids, decode_list, encode_ids, length,
};

pub(super) fn encode(
    bytes: &mut Vec<u8>,
    provider: &ProviderCandidateConformance,
    retain_projected_qualifications: bool,
) {
    bytes.extend_from_slice(&provider.boundary.get().to_le_bytes());
    encode_string(bytes, &provider.requirement_identity);
    encode_string(bytes, &provider.provider_identity);
    encode_string(bytes, &provider.candidate_identity);
    bytes.extend_from_slice(&provider.candidate.get().to_le_bytes());
    length(bytes, provider.signature.parameters.len());
    for parameter in &provider.signature.parameters {
        bytes.extend_from_slice(&parameter.position.to_le_bytes());
        bytes.extend_from_slice(&parameter.structural_type.get().to_le_bytes());
        encode_multiplicity(bytes, parameter.multiplicity);
        encode_access(bytes, parameter.access);
        encode_ids(
            bytes,
            parameter.qualifications.iter().map(|value| value.get()),
        );
        encode_projected(
            bytes,
            &parameter.projected_qualifications,
            retain_projected_qualifications,
        );
    }
    length(bytes, provider.refinement.positional_parameters.len());
    for parameter in &provider.refinement.positional_parameters {
        bytes.extend_from_slice(&parameter.boundary_index.to_le_bytes());
        bytes.extend_from_slice(&parameter.candidate_index.to_le_bytes());
    }
    length(bytes, provider.refinement.required_domains.len());
    for requirement in &provider.refinement.required_domains {
        bytes.extend_from_slice(&requirement.argument_index.to_le_bytes());
        bytes.extend_from_slice(&requirement.domain.get().to_le_bytes());
    }
    encode_ids(
        bytes,
        provider
            .refinement
            .realized_service_ceiling
            .iter()
            .map(|value| value.get()),
    );
}

pub(super) fn decode(
    cursor: &mut Cursor<'_>,
    retain_projected_qualifications: bool,
) -> Result<ProviderCandidateConformance, FixedViewCopyDecodeError> {
    let boundary = decode_id(cursor, BoundaryMachineId::new)?;
    let requirement_identity = decode_string(cursor)?;
    let provider_identity = decode_string(cursor)?;
    let candidate_identity = decode_string(cursor)?;
    let candidate = decode_id(cursor, MachineId::new)?;
    let parameters = decode_list(cursor, |cursor| {
        Ok(ProviderSignatureParameter {
            position: cursor.u32()?,
            structural_type: decode_id(cursor, StructuralTypeId::new)?,
            multiplicity: decode_multiplicity(cursor)?,
            access: decode_access(cursor)?,
            qualifications: decode_ids(cursor, semantic_vocabulary::StructuralDomainId::new)?,
            projected_qualifications: decode_projected(cursor, retain_projected_qualifications)?,
        })
    })?;
    let positional_parameters = decode_list(cursor, |cursor| {
        Ok(ProviderParameterRefinement {
            boundary_index: cursor.u32()?,
            candidate_index: cursor.u32()?,
        })
    })?;
    let required_domains = decode_list(cursor, |cursor| {
        Ok(StructuralDomainRequirement {
            argument_index: cursor.u32()?,
            domain: decode_id(cursor, semantic_vocabulary::StructuralDomainId::new)?,
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
            realized_service_ceiling: decode_ids(cursor, ServiceId::new)?,
        },
    })
}
