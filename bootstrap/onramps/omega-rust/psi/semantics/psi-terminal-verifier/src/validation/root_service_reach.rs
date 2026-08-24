use super::*;

/// Reconstruct the selected entry's exact executable service closure.
///
/// Concrete reach and provider-selected installation dependencies are different
/// axes. A concrete operation continues to contribute its service even when
/// that service also appears in an abstract dependency's upper bound; nothing
/// is recovered by subtracting bounds from published machine ceilings.
pub(super) fn validate_root_service_reach_exact(
    module: &TerminalModule,
) -> Result<(), ModuleError> {
    let machines = module
        .machines
        .iter()
        .map(|machine| (machine.id, machine))
        .collect::<BTreeMap<_, _>>();
    let boundaries = module
        .boundary_machines
        .iter()
        .map(|boundary| (boundary.id, boundary))
        .collect::<BTreeMap<_, _>>();
    let services = module
        .services
        .iter()
        .map(|service| (service.id, service))
        .collect::<BTreeMap<_, _>>();
    let dependencies = module
        .root_service_reach
        .installation_dependencies
        .iter()
        .map(|dependency| (dependency.requirement_identity.as_str(), dependency))
        .collect::<BTreeMap<_, _>>();

    let mut pending = vec![module.entry];
    let mut visited = BTreeSet::new();
    let mut concrete = BTreeSet::new();
    let mut used_dependencies = BTreeSet::new();

    while let Some(machine_id) = pending.pop() {
        if !visited.insert(machine_id) {
            continue;
        }
        let machine = machines
            .get(&machine_id)
            .copied()
            .ok_or(ModuleError::UnknownEntryMachine(machine_id))?;
        for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
            match &operation.kind {
                OperationKind::Call { callee, .. }
                | OperationKind::CallUnit { callee, .. }
                | OperationKind::CallStructuralScalar { callee, .. }
                | OperationKind::CallStructural { callee, .. } => pending.push(*callee),
                OperationKind::BoundaryCall { boundary, .. } => {
                    let declaration = boundaries.get(boundary).copied().ok_or(
                        ModuleError::UnknownBoundaryCallTarget {
                            operation: operation.id,
                            boundary: *boundary,
                        },
                    )?;
                    if let Some(dependency) = dependencies.get(declaration.identity.as_str()) {
                        if declaration.published_service_ceiling != dependency.upper_bound {
                            return Err(ModuleError::InstallationReachBoundaryMismatch(*boundary));
                        }
                        used_dependencies.insert(declaration.identity.as_str());
                    } else {
                        concrete.extend(declaration.published_service_ceiling.iter().copied());
                    }
                }
                OperationKind::PortWrite { service, .. } => {
                    concrete.insert(*service);
                    if let Some(declaration) = services.get(service) {
                        concrete.extend(declaration.parents.iter().copied());
                    }
                }
                _ => {}
            }
        }
    }

    let derived_concrete = concrete.into_iter().collect::<Vec<_>>();
    if derived_concrete != module.root_service_reach.concrete {
        return Err(ModuleError::RootConcreteServiceReachMismatch {
            declared: module.root_service_reach.concrete.clone(),
            derived: derived_concrete,
        });
    }
    let declared_dependencies = dependencies.keys().copied().collect::<BTreeSet<_>>();
    if used_dependencies != declared_dependencies {
        return Err(ModuleError::RootInstallationReachDependenciesMismatch);
    }
    Ok(())
}
