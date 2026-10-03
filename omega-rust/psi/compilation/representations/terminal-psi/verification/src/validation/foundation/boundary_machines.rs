//! The module's boundary machines: their references, attachments,
//! parameters, requirements and results.

use super::super::{
    BTreeMap, BTreeSet, BoundaryMachineDeclaration, BoundaryMachineResult, ModuleError,
    PropositionContext, ServiceId, StructuralDomainId, StructuralTypeId, TerminalModule,
};
use super::{
    ServiceCeilingOwner, StructuralSignatureOwner, validate_attachment,
    validate_program_local_root_introductions, validate_service_ceiling,
    validate_structural_signature,
};
use terminal_psi::{ServiceDeclaration, StructuralDomainDeclaration, StructuralTypeDeclaration};

/// Validates the module's boundary machines: ids and names are unique,
/// reference types, attachments, structural parameters, requirements and
/// results name known types, domains and services.
pub(super) fn validate_boundary_machines(
    module: &TerminalModule,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    domains: &BTreeMap<StructuralDomainId, &StructuralDomainDeclaration>,
    services: &BTreeMap<ServiceId, &ServiceDeclaration>,
) -> Result<(), ModuleError> {
    let mut boundary_ids = BTreeSet::new();
    let mut boundary_names = BTreeSet::new();
    for boundary in &module.boundary_machines {
        // Until terminal external invocations exist, the only retained external
        // contract is the transitional Unit process-exit requirement. It never
        // acquires normal guarantees or crash alternatives from this encoding.
        if matches!(
            boundary.completion,
            terminal_psi::BoundaryCompletion::External(_)
        ) && (!boundary.result.is_unit()
            || !boundary.crash_routes.is_empty()
            || !boundary.scalar_ensures.is_empty()
            || !boundary.content_guarantees.is_empty())
        {
            return Err(ModuleError::InvalidBoundaryMachineIdentity(boundary.id));
        }
        if !boundary.has_valid_parameter_order() {
            return Err(ModuleError::InvalidBoundaryParameterOrder(boundary.id));
        }
        let reference_type = boundary
            .structural_parameters
            .iter()
            .map(|parameter| parameter.structural_type)
            .chain(match &boundary.result {
                BoundaryMachineResult::Structural(result) => Some(result.structural_type),
                _ => None,
            })
            .find(|structural_type| {
                super::super::references::contains_reference(module, *structural_type)
            });
        if let Some(structural_type) = reference_type {
            // Boundary result signatures do not yet carry reference-source
            // contracts, so they cannot transfer or manufacture loan custody.
            return Err(ModuleError::InvalidStructuralTypeIdentity(structural_type));
        }
        if !boundary_ids.insert(boundary.id) {
            return Err(ModuleError::DuplicateBoundaryMachine(boundary.id));
        }
        if boundary.identity.is_empty() || !boundary_names.insert(boundary.identity.as_str()) {
            return Err(ModuleError::InvalidBoundaryMachineIdentity(boundary.id));
        }
        super::super::crash::validate_boundary_crash_routes(boundary)?;
        validate_boundary_scalar_contract(boundary, false)?;
        validate_boundary_scalar_contract(boundary, true)?;
        if boundary.kind != terminal_psi::BoundaryRequirementKind::Machine
            && boundary.attachment.is_some()
        {
            return Err(ModuleError::InvalidBoundaryMachineIdentity(boundary.id));
        }
        validate_attachment(boundary.id, boundary.attachment, types)?;
        validate_structural_signature(
            &boundary.structural_parameters,
            boundary.attachment,
            types,
            domains,
            StructuralSignatureOwner::Boundary(boundary.id),
        )?;
        validate_service_ceiling(
            &boundary.published_service_ceiling,
            services,
            ServiceCeilingOwner::Boundary(boundary.id),
        )?;
        validate_service_ceiling(
            &boundary.fixed_service_reach,
            services,
            ServiceCeilingOwner::BoundaryFixed(boundary.id),
        )?;
        if let Some(service) = boundary
            .fixed_service_reach
            .iter()
            .find(|service| !boundary.published_service_ceiling.contains(service))
        {
            return Err(ModuleError::FixedBoundaryServiceOutsidePublishedCeiling {
                boundary: boundary.id,
                service: *service,
            });
        }
        let mut requirements = BTreeSet::new();
        for requirement in &boundary.requires {
            if !requirements.insert(*requirement) {
                return Err(ModuleError::DuplicateBoundaryRequirement {
                    boundary: boundary.id,
                    argument_index: requirement.argument_index,
                    domain: requirement.domain,
                });
            }
            let Some(parameter) = boundary
                .structural_parameters
                .get(requirement.argument_index as usize)
            else {
                return Err(ModuleError::BoundaryRequirementArgumentOutOfRange {
                    boundary: boundary.id,
                    argument_index: requirement.argument_index,
                });
            };
            let Some(domain) = domains.get(&requirement.domain) else {
                return Err(ModuleError::UnknownStructuralDomain(requirement.domain));
            };
            if domain.carrier != parameter.structural_type {
                return Err(ModuleError::StructuralDomainCarrierMismatch {
                    domain: domain.id,
                    expected: parameter.structural_type,
                    actual: domain.carrier,
                });
            }
        }
        if boundary.requires.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(ModuleError::NonCanonicalBoundaryRequirements(boundary.id));
        }
        validate_program_local_root_introductions(boundary, types, domains)?;
    }
    Ok(())
}

/// Boundary scalar contracts use their declaration-local telescope. Normal
/// guarantees additionally admit the declared result coordinate. Structural
/// places, opaque identities and values from callers or providers are excluded;
/// each call substitutes its actuals only after this namespace is validated.
fn validate_boundary_scalar_contract(
    boundary: &BoundaryMachineDeclaration,
    normal_return: bool,
) -> Result<(), ModuleError> {
    let rows = if normal_return {
        &boundary.scalar_ensures
    } else {
        &boundary.scalar_requires
    };
    if rows.is_empty() {
        return Ok(());
    }
    let mut parameters = boundary
        .scalar_contract_parameters()
        .ok_or(ModuleError::InvalidBoundaryCrashParameters(boundary.id))?;
    if normal_return && let Some(result) = boundary.scalar_contract_result() {
        parameters.push(result);
    }
    let context = PropositionContext::from_value_types(
        parameters
            .iter()
            .map(|parameter| (parameter.id, parameter.scalar_type)),
    )
    .map_err(ModuleError::MalformedProposition)?;
    for proposition in rows {
        if !proposition.visit_value_ids(|_| {}) {
            return Err(ModuleError::UnsupportedBoundaryRequiresProposition(
                boundary.id,
            ));
        }
        context
            .validate(proposition)
            .map_err(ModuleError::MalformedProposition)?;
    }
    Ok(())
}
