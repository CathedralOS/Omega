//! Installed-provider signature and refinement wire custody.

use semantic_vocabulary::{BoundaryMachineId, MachineId, ServiceId, StructuralTypeId};
use terminal_psi::{
    ProviderCandidateConformance, ProviderParameterRefinement, ProviderRefinement,
    ProviderSignature, ProviderSignatureParameter, StructuralDomainRequirement,
};

use crate::FixedViewCopyDecodeError;

use super::{
    declarations::{decode_access, decode_multiplicity, encode_access, encode_multiplicity},
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
    crate::codec_bytes::push_encoded(bytes, provider.boundary.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, provider.requirement_identity.0.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, provider.source_requirement_identity.0.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, provider.receiving_application.0.to_le_bytes());
    length(bytes, provider.receiving_region_arguments.len());
    for region in &provider.receiving_region_arguments {
        crate::codec_bytes::push_encoded(bytes, region.0.to_le_bytes());
    }
    crate::codec_bytes::push_encoded(bytes, provider.provider_identity.0.to_le_bytes());
    crate::codec_bytes::push_encoded(
        bytes,
        provider.provider_declaration.identity().0.to_le_bytes(),
    );
    crate::codec_bytes::push_encoded(bytes, provider.candidate_identity.0.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, provider.source_candidate_identity.0.to_le_bytes());
    crate::codec_bytes::option(
        bytes,
        provider.source_candidate_package_identity,
        |bytes, package| {
            bytes.extend_from_slice(&package.digest());
        },
    );
    crate::codec_bytes::push_encoded(bytes, provider.candidate.get().to_le_bytes());
    length(bytes, provider.signature.parameters.len());
    for parameter in &provider.signature.parameters {
        crate::codec_bytes::push_encoded(bytes, parameter.position.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, parameter.structural_type.get().to_le_bytes());
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
        crate::codec_bytes::push_encoded(bytes, parameter.boundary_index.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, parameter.candidate_index.to_le_bytes());
    }
    length(bytes, provider.refinement.required_domains.len());
    for requirement in &provider.refinement.required_domains {
        crate::codec_bytes::push_encoded(bytes, requirement.argument_index.to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, requirement.domain.get().to_le_bytes());
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
    let requirement_identity = terminal_psi::TerminalIdentityIndex(cursor.u32()?);
    let source_requirement_identity = terminal_psi::TerminalIdentityIndex(cursor.u32()?);
    let receiving_application = terminal_psi::TerminalIdentityIndex(cursor.u32()?);
    let receiving_region_arguments =
        decode_list::<_, FixedViewCopyDecodeError>(cursor, |cursor| {
            Ok(terminal_psi::TerminalIdentityIndex(cursor.u32()?))
        })?;
    let provider_identity = terminal_psi::TerminalIdentityIndex(cursor.u32()?);
    let provider_declaration = terminal_psi::NominalDeclarationIdentity::new(
        terminal_psi::TerminalIdentityIndex(cursor.u32()?),
    );
    let candidate_identity = terminal_psi::TerminalIdentityIndex(cursor.u32()?);
    let source_candidate_identity = terminal_psi::TerminalIdentityIndex(cursor.u32()?);
    let source_candidate_package_identity = crate::cursor::optional(
        cursor,
        |cursor| {
            semantic_vocabulary::PackageKeyIdentity::from_digest(cursor.array()?)
                .ok_or(FixedViewCopyDecodeError::InvalidProviderExecution)
        },
        FixedViewCopyDecodeError::UnknownOption,
    )?;
    let candidate = decode_id(cursor, MachineId::new)?;
    let parameters = decode_list::<_, FixedViewCopyDecodeError>(cursor, |cursor| {
        Ok(ProviderSignatureParameter {
            position: cursor.u32()?,
            structural_type: decode_id(cursor, StructuralTypeId::new)?,
            multiplicity: decode_multiplicity(cursor)?,
            access: decode_access(cursor)?,
            qualifications: decode_ids(cursor, semantic_vocabulary::StructuralDomainId::new)?,
            projected_qualifications: decode_projected(cursor, retain_projected_qualifications)?,
        })
    })?;
    let positional_parameters = decode_list::<_, FixedViewCopyDecodeError>(cursor, |cursor| {
        Ok(ProviderParameterRefinement {
            boundary_index: cursor.u32()?,
            candidate_index: cursor.u32()?,
        })
    })?;
    let required_domains = decode_list::<_, FixedViewCopyDecodeError>(cursor, |cursor| {
        Ok(StructuralDomainRequirement {
            argument_index: cursor.u32()?,
            domain: decode_id(cursor, semantic_vocabulary::StructuralDomainId::new)?,
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
            realized_service_ceiling: decode_ids(cursor, ServiceId::new)?,
        },
    })
}
