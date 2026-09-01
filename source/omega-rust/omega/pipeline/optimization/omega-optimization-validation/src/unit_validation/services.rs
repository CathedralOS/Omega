//! Service catalogs, root reachability, and structural signatures.

use super::*;

pub(crate) fn index_service_catalog(
    unit: &PsiOptimizationUnit,
) -> Result<BTreeMap<ServiceId, &psi_terminal::ServiceDeclaration>, OptimizationUnitValidationError>
{
    let mut services = BTreeMap::new();
    let mut identities = BTreeSet::new();
    for declaration in unit.services.iter() {
        if services.insert(declaration.id, declaration).is_some() {
            return Err(OptimizationUnitValidationError::DuplicateService(
                declaration.id,
            ));
        }
        if declaration.identity.is_empty() || !identities.insert(declaration.identity.as_str()) {
            return Err(OptimizationUnitValidationError::InvalidServiceIdentity(
                declaration.id,
            ));
        }
    }
    for declaration in unit.services.iter() {
        let mut parents = BTreeSet::new();
        for parent in &declaration.parents {
            if *parent == declaration.id
                || !parents.insert(*parent)
                || !services.contains_key(parent)
            {
                return Err(OptimizationUnitValidationError::InvalidServiceParent {
                    service: declaration.id,
                    parent: *parent,
                });
            }
        }
        if declaration
            .parents
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(OptimizationUnitValidationError::NonCanonicalServiceParents(
                declaration.id,
            ));
        }
    }

    fn visit(
        id: ServiceId,
        services: &BTreeMap<ServiceId, &psi_terminal::ServiceDeclaration>,
        active: &mut BTreeSet<ServiceId>,
        complete: &mut BTreeSet<ServiceId>,
    ) -> Result<(), OptimizationUnitValidationError> {
        if complete.contains(&id) {
            return Ok(());
        }
        if !active.insert(id) {
            return Err(OptimizationUnitValidationError::RecursiveServiceHierarchy(
                id,
            ));
        }
        for parent in &services[&id].parents {
            visit(*parent, services, active, complete)?;
        }
        active.remove(&id);
        complete.insert(id);
        Ok(())
    }

    let mut active = BTreeSet::new();
    let mut complete = BTreeSet::new();
    for id in services.keys().copied() {
        visit(id, &services, &mut active, &mut complete)?;
    }
    for declaration in services.values() {
        for parent in &declaration.parents {
            if let Some(ancestor) = services[parent]
                .parents
                .iter()
                .find(|ancestor| !declaration.parents.contains(ancestor))
            {
                return Err(
                    OptimizationUnitValidationError::IncompleteServiceParentClosure {
                        service: declaration.id,
                        ancestor: *ancestor,
                    },
                );
            }
        }
    }
    Ok(services)
}

pub(crate) fn valid_service_ceiling(
    ceiling: &[ServiceId],
    services: &BTreeMap<ServiceId, &psi_terminal::ServiceDeclaration>,
) -> bool {
    let mut seen = BTreeSet::new();
    ceiling.iter().all(|service| {
        seen.insert(*service)
            && services.get(service).is_some_and(|declaration| {
                declaration
                    .parents
                    .iter()
                    .all(|parent| ceiling.contains(parent))
            })
    }) && ceiling.windows(2).all(|pair| pair[0] < pair[1])
}

pub(crate) fn validate_root_service_reach(
    unit: &PsiOptimizationUnit,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundaries: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
    services: &BTreeMap<ServiceId, &psi_terminal::ServiceDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    if !valid_service_ceiling(&unit.root_service_reach.concrete, services) {
        return Err(OptimizationUnitValidationError::InvalidRootConcreteServiceReach);
    }
    let mut requirement_identities = BTreeSet::new();
    for (index, dependency) in unit
        .root_service_reach
        .installation_dependencies
        .iter()
        .enumerate()
    {
        if dependency.requirement_identity.is_empty()
            || !requirement_identities.insert(dependency.requirement_identity.as_str())
            || !valid_service_ceiling(&dependency.upper_bound, services)
        {
            return Err(
                OptimizationUnitValidationError::InvalidRootInstallationReachDependency(index),
            );
        }
    }
    if unit
        .root_service_reach
        .installation_dependencies
        .windows(2)
        .any(|pair| pair[0].requirement_identity >= pair[1].requirement_identity)
    {
        return Err(OptimizationUnitValidationError::NonCanonicalRootInstallationReachDependencies);
    }
    let derived = derive_root_service_reach(unit, functions, boundaries, services)?;
    if derived.concrete != unit.root_service_reach.concrete {
        return Err(
            OptimizationUnitValidationError::RootConcreteServiceReachMismatch {
                declared: unit.root_service_reach.concrete.clone(),
                derived: derived.concrete,
            },
        );
    }
    if derived.installation_dependencies != unit.root_service_reach.installation_dependencies {
        return Err(OptimizationUnitValidationError::RootInstallationReachDependenciesMismatch);
    }
    Ok(())
}

pub(crate) fn derive_root_service_reach(
    unit: &PsiOptimizationUnit,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundaries: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
    services: &BTreeMap<ServiceId, &psi_terminal::ServiceDeclaration>,
) -> Result<psi_terminal::TerminalRootServiceReach, OptimizationUnitValidationError> {
    let dependencies = unit
        .root_service_reach
        .installation_dependencies
        .iter()
        .map(|dependency| (dependency.requirement_identity.as_str(), dependency))
        .collect::<BTreeMap<_, _>>();
    let mut pending = vec![unit.entry];
    let mut visited = BTreeSet::new();
    let mut concrete = BTreeSet::new();
    let mut used_dependencies = BTreeSet::new();
    while let Some(machine) = pending.pop() {
        if !visited.insert(machine) {
            continue;
        }
        let function = functions.get(&machine).copied().ok_or(
            OptimizationUnitValidationError::MissingEntryMachine(machine),
        )?;
        for operation in function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .map(|node| &node.operation)
        {
            match operation {
                O::Call { callee, .. }
                | O::CallUnit { callee, .. }
                | O::CallStructuralScalar { callee, .. }
                | O::CallStructural { callee, .. } => pending.push(*callee),
                O::CallDynamicScalar {
                    dynamic_dispatch, ..
                } => pending.push(dynamic_dispatch.dispatch.realization),
                O::BoundaryCall { boundary, .. } => {
                    let declaration = boundaries.get(boundary).copied().ok_or(
                        OptimizationUnitValidationError::OperationServiceContractMismatch {
                            machine: function.machine,
                            block: function.entry,
                            node: 0,
                        },
                    )?;
                    if let Some(dependency) = dependencies.get(declaration.identity.as_str()) {
                        if declaration.published_service_ceiling != dependency.upper_bound {
                            return Err(
                                OptimizationUnitValidationError::RootInstallationReachBoundaryMismatch(
                                    *boundary,
                                ),
                            );
                        }
                        used_dependencies.insert(declaration.identity.as_str());
                    } else {
                        concrete.extend(declaration.published_service_ceiling.iter().copied());
                    }
                }
                O::PortWrite { service, .. } => {
                    concrete.insert(*service);
                    if let Some(declaration) = services.get(service) {
                        concrete.extend(declaration.parents.iter().copied());
                    }
                }
                _ => {}
            }
        }
    }
    let installation_dependencies = unit
        .root_service_reach
        .installation_dependencies
        .iter()
        .filter(|dependency| used_dependencies.contains(dependency.requirement_identity.as_str()))
        .cloned()
        .collect();
    Ok(psi_terminal::TerminalRootServiceReach {
        concrete: concrete.into_iter().collect(),
        installation_dependencies,
    })
}

pub(crate) fn refresh_root_service_reach(
    unit: &mut PsiOptimizationUnit,
) -> Result<(), OptimizationUnitValidationError> {
    let reach = {
        let functions = unit
            .functions
            .iter()
            .map(|function| (function.machine, function))
            .collect::<BTreeMap<_, _>>();
        let boundaries = unit
            .boundary_machines
            .iter()
            .map(|boundary| (boundary.id, boundary))
            .collect::<BTreeMap<_, _>>();
        let services = unit
            .services
            .iter()
            .map(|service| (service.id, service))
            .collect::<BTreeMap<_, _>>();
        derive_root_service_reach(unit, &functions, &boundaries, &services)?
    };
    unit.root_service_reach = reach;
    Ok(())
}

pub(crate) fn validate_provider_service_refinements(
    unit: &PsiOptimizationUnit,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    boundaries: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    for provider in &unit.provider_candidates {
        let invalid = || OptimizationUnitValidationError::InvalidProviderServiceRefinement {
            boundary: provider.boundary,
            candidate: provider.candidate,
        };
        let candidate = functions.get(&provider.candidate).ok_or_else(invalid)?;
        let boundary = boundaries.get(&provider.boundary).ok_or_else(invalid)?;
        if provider.refinement.realized_service_ceiling != candidate.published_service_ceiling
            || provider
                .refinement
                .realized_service_ceiling
                .iter()
                .any(|service| !boundary.published_service_ceiling.contains(service))
        {
            return Err(invalid());
        }
    }
    Ok(())
}

pub(crate) fn boundary_structural_signature_matches(
    boundary: &psi_terminal::BoundaryMachineDeclaration,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
    domains: &BTreeMap<StructuralDomainId, &psi_terminal::StructuralDomainDeclaration>,
) -> bool {
    structural_signature_matches(
        &boundary.structural_parameters,
        boundary.attachment,
        types,
        domains,
    ) && boundary.requires.windows(2).all(|pair| pair[0] < pair[1])
        && boundary.requires.iter().all(|requirement| {
            boundary
                .structural_parameters
                .get(requirement.argument_index as usize)
                .is_some_and(|parameter| {
                    domains
                        .get(&requirement.domain)
                        .is_some_and(|domain| domain.carrier == parameter.structural_type)
                })
        })
}

/// Replay Terminal's exact attachment/self half of a structural signature.
/// An attachment need not have a runtime `self` parameter (provider-backed
/// specializations deliberately do not), but every retained `self` must be the
/// unique parameter whose type is that attachment.
pub(crate) fn structural_signature_matches(
    parameters: &[psi_terminal::StructuralParameterDeclaration],
    attachment: Option<StructuralTypeId>,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
    domains: &BTreeMap<StructuralDomainId, &psi_terminal::StructuralDomainDeclaration>,
) -> bool {
    if attachment.is_some_and(|attachment| !types.contains_key(&attachment)) {
        return false;
    }
    let mut places = BTreeSet::new();
    let mut saw_self = false;
    parameters.iter().enumerate().all(|(position, parameter)| {
        let self_matches = if parameter.is_self {
            let matches = !saw_self && attachment == Some(parameter.structural_type);
            saw_self = true;
            matches
        } else {
            true
        };
        u32::try_from(position).ok() == Some(parameter.position)
            && places.insert(parameter.place)
            && types.contains_key(&parameter.structural_type)
            && self_matches
            && structural_qualifications_match(
                parameter.structural_type,
                &parameter.qualifications,
                domains,
            )
    })
}
