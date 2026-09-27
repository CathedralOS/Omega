//! Module-level declarations: structural domains, services and their
//! reach, boundary machines and provider candidates.

use super::{
    has_service, has_structural_type, require_known_services, validate_service_parent_graph,
    validate_structural_parameters,
};
use crate::codec_error::{CodecError, malformed};
use terminal_psi::TerminalModule;
use terminal_psi::{BoundaryMachineDeclaration, ProviderCandidateConformance};

/// Every structural domain names a known carrier type.
pub(super) fn validate_domains(module: &TerminalModule) -> Result<(), CodecError> {
    for domain in &module.structural_domains {
        if !has_structural_type(module, domain.carrier) {
            return malformed("structural domain references an unknown carrier type");
        }
    }
    Ok(())
}

/// Every service names known parents other than itself, the parent graph
/// is well founded, and the root service reach and its installation
/// dependencies name known services.
pub(super) fn validate_services(module: &TerminalModule) -> Result<(), CodecError> {
    for service in &module.services {
        if service
            .parents
            .iter()
            .any(|parent| *parent == service.id || !has_service(module, *parent))
        {
            return malformed("service references itself or an unknown parent");
        }
    }
    validate_service_parent_graph(module)?;
    require_known_services(module, &module.root_service_reach.concrete)?;
    for dependency in &module.root_service_reach.installation_dependencies {
        if dependency.requirement_identity.is_empty() {
            return malformed("installation reach requirement identity must be nonempty");
        }
        require_known_services(module, &dependency.upper_bound)?;
    }
    Ok(())
}

/// One boundary machine's attachment, structural parameters,
/// requirements and service reach name known types, domains and services.
pub(super) fn validate_boundary_machine(
    module: &TerminalModule,
    boundary: &BoundaryMachineDeclaration,
) -> Result<(), CodecError> {
    if boundary
        .attachment
        .is_some_and(|attachment| !has_structural_type(module, attachment))
    {
        return malformed("boundary machine has an unknown attachment type");
    }
    validate_structural_parameters(module, &boundary.structural_parameters)?;
    if !boundary.has_valid_parameter_order() {
        return malformed("boundary parameter order does not cover its lanes exactly");
    }
    for requirement in &boundary.requires {
        let Some(parameter) = boundary
            .structural_parameters
            .get(requirement.argument_index as usize)
        else {
            return malformed("boundary requirement has an unknown argument index");
        };
        let Some(domain) = module
            .structural_domains
            .iter()
            .find(|domain| domain.id == requirement.domain)
        else {
            return malformed("boundary requirement references an unknown domain");
        };
        if domain.carrier != parameter.structural_type {
            return malformed("boundary requirement domain has the wrong carrier type");
        }
    }
    require_known_services(module, &boundary.published_service_ceiling)?;
    require_known_services(module, &boundary.fixed_service_reach)?;
    Ok(())
}

/// One provider candidate carries nonempty identities and names a known
/// boundary, candidate machine and signature types.
pub(super) fn validate_provider_candidate(
    module: &TerminalModule,
    candidate: &ProviderCandidateConformance,
) -> Result<(), CodecError> {
    if candidate.requirement_identity.is_empty()
        || candidate.provider_identity.is_empty()
        || candidate.candidate_identity.is_empty()
    {
        return malformed("provider candidate identities must be nonempty");
    }
    if !module
        .boundary_machines
        .iter()
        .any(|boundary| boundary.id == candidate.boundary)
        || !module
            .machines
            .iter()
            .any(|machine| machine.id == candidate.candidate)
    {
        return malformed("provider candidate references an unknown terminal ID");
    }
    for parameter in &candidate.signature.parameters {
        if !has_structural_type(module, parameter.structural_type) {
            return malformed("provider signature references an unknown structural type");
        }
        if parameter.qualifications.iter().any(|domain| {
            !module
                .structural_domains
                .iter()
                .any(|row| row.id == *domain)
        }) {
            return malformed("provider signature references an unknown structural domain");
        }
    }
    require_known_services(module, &candidate.refinement.realized_service_ceiling)?;
    Ok(())
}
