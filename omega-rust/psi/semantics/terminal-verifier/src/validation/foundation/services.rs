//! The module's services, their graph and the root service reach.

use super::super::{BTreeMap, BTreeSet, ModuleError, ServiceId, TerminalModule};
use super::{ServiceCeilingOwner, validate_service_ceiling, validate_service_graph};
use terminal_psi::ServiceDeclaration;

/// Registers the module's services: ids and names are unique, parents are
/// known, the service graph is well founded, the root service reach names
/// known services, and its installation dependencies are canonical.
pub(super) fn register_services(
    module: &TerminalModule,
) -> Result<BTreeMap<ServiceId, &ServiceDeclaration>, ModuleError> {
    let mut services = BTreeMap::new();
    let mut service_names = BTreeSet::new();
    for declaration in &module.services {
        if services.insert(declaration.id, declaration).is_some() {
            return Err(ModuleError::DuplicateService(declaration.id));
        }
        if declaration.identity.is_empty() || !service_names.insert(declaration.identity.as_str()) {
            return Err(ModuleError::InvalidServiceIdentity(declaration.id));
        }
    }
    for declaration in &module.services {
        let mut parents = BTreeSet::new();
        for parent in &declaration.parents {
            if *parent == declaration.id
                || !parents.insert(*parent)
                || !services.contains_key(parent)
            {
                return Err(ModuleError::InvalidServiceParent {
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
            return Err(ModuleError::NonCanonicalServiceParents(declaration.id));
        }
    }
    validate_service_graph(&services)?;

    validate_service_ceiling(
        &module.root_service_reach.concrete,
        &services,
        ServiceCeilingOwner::RootConcrete,
    )?;

    let mut installation_requirements = BTreeSet::new();
    for (index, dependency) in module
        .root_service_reach
        .installation_dependencies
        .iter()
        .enumerate()
    {
        if dependency.requirement_identity.is_empty()
            || !installation_requirements.insert(dependency.requirement_identity.as_str())
        {
            return Err(ModuleError::InvalidInstallationReachDependency(index));
        }
        validate_service_ceiling(
            &dependency.upper_bound,
            &services,
            ServiceCeilingOwner::InstallationReach(index),
        )?;
    }
    if module
        .root_service_reach
        .installation_dependencies
        .windows(2)
        .any(|pair| pair[0].requirement_identity >= pair[1].requirement_identity)
    {
        return Err(ModuleError::NonCanonicalInstallationReachDependencies);
    }
    Ok(services)
}
