//! Collect the reached service and installation requirements, then check an explicitly supplied provider closure.
use super::SynchronousCallGraph;
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use std::collections::BTreeSet;
use terminal_psi::{OperationKind, TargetMachineCallable};

impl SynchronousCallGraph<'_> {
    /// Describe the services of the ordinary call closure while its
    /// installation-bound calls still name their exact requirements.
    pub fn service_reach_from(
        &self,
        root: MachineId,
        installation_requirements: &[BoundaryMachineId],
    ) -> Result<terminal_psi::TerminalRootServiceReach, String> {
        let closure = self.visit_callables_stopping_at(
            [TargetMachineCallable::Machine(root)],
            &mut BTreeSet::new(),
            installation_requirements,
        )?;
        let mut concrete = BTreeSet::new();
        let mut installation_dependencies = Vec::new();
        for body in closure.machines {
            concrete.extend(body.declared_service_reach.iter().flatten().copied());
            for operation in body.operations() {
                let service = match operation.kind {
                    OperationKind::PortWrite { service, .. } => Some(service),
                    OperationKind::RegisterAccess { service, .. } => service,
                    _ => None,
                };
                if let Some(service) = service {
                    let row = self.module.service(service).ok_or_else(|| {
                        format!("service reach lost instruction service {service:?}")
                    })?;
                    concrete.insert(service);
                    concrete.extend(row.parents.iter().copied());
                }
            }
        }
        for called in closure.boundaries {
            let required = self
                .module
                .boundary_machine(called)
                .ok_or_else(|| format!("service reach lost boundary {called:?}"))?;
            if installation_requirements.binary_search(&called).is_ok() {
                if !required.service_reach_is_installation_bound {
                    return Err("installation reach names a fixed requirement".into());
                }
                installation_dependencies.push(terminal_psi::InstallationReachDependency {
                    requirement_identity: required.identity,
                    upper_bound: required.published_service_ceiling.clone(),
                });
            } else if required.service_reach_is_installation_bound {
                if self.boundary_callee(called)?.is_none() {
                    return Err("installation reach has no checked selected provider".into());
                }
            } else {
                concrete.extend(required.published_service_ceiling.iter().copied());
            }
        }
        installation_dependencies.sort_by_key(|row| row.requirement_identity);
        if installation_dependencies
            .windows(2)
            .any(|pair| pair[0].requirement_identity == pair[1].requirement_identity)
        {
            return Err("installation reach repeats an exact requirement identity".into());
        }
        Ok(terminal_psi::TerminalRootServiceReach {
            concrete: concrete.into_iter().collect(),
            installation_dependencies,
        })
    }

    /// Check the actual selected rows before executing this installed root.
    /// Symbolic child rows are resolved once in dependency order; an absent
    /// child or cycle cannot establish a parent's finite bound.
    pub fn validate_installed_service_reach(&self, root: MachineId) -> Result<(), String> {
        use graphs::reach::{ReachResolutionError, ReachRow, resolve};
        let closure = self.from_root(root)?;
        let mut required = Vec::new();
        for id in &closure.boundaries {
            let boundary = self
                .module
                .boundary_machine(*id)
                .ok_or("installed reach lost a boundary declaration")?;
            if boundary.service_reach_is_installation_bound {
                required.push(boundary);
            }
        }
        if required.is_empty() {
            return Ok(());
        }
        let mut positions = required
            .iter()
            .enumerate()
            .map(|(position, boundary)| (boundary.identity, position))
            .collect::<Vec<_>>();
        positions.sort_unstable();
        if positions.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err("installed reach has competing exact requirement identities".into());
        }
        let requirements = self.module.installation_reach_requirements();
        let mut rows = Vec::with_capacity(required.len());
        for boundary in &required {
            let selected = self
                .boundary_callee(boundary.id)?
                .ok_or("installation reach has no checked selected provider")?;
            let reach = self.service_reach_from(selected, &requirements)?;
            let dependencies = reach
                .installation_dependencies
                .iter()
                .map(|dependency| {
                    positions
                        .binary_search_by_key(&dependency.requirement_identity, |row| row.0)
                        .ok()
                        .map(|position| positions[position].1)
                })
                .collect();
            rows.push(ReachRow {
                fixed: reach.concrete,
                dependencies,
            });
        }
        let resolved = resolve(rows).map_err(|error| match error {
            ReachResolutionError::Unresolved(pending) => {
                let identities = pending
                    .iter()
                    .map(|(position, _)| {
                        self.module
                            .identities
                            .text(required[*position].identity)
                            .unwrap_or("<absent requirement identity>")
                    })
                    .collect::<Vec<_>>();
                format!(
                    "selected installation retains unresolved or cyclic reach rows: {}",
                    identities.join(", ")
                )
            }
            ReachResolutionError::InvalidDependency { row, dependency } => {
                format!("installed reach row {row} has invalid dependency {dependency}")
            }
        })?;
        for (boundary, row) in required.iter().zip(resolved) {
            if row.iter().any(|service| {
                boundary
                    .published_service_ceiling
                    .binary_search(service)
                    .is_err()
            }) {
                let identity = self
                    .module
                    .identities
                    .text(boundary.identity)
                    .ok_or("installed reach lost its exact requirement identity")?;
                return Err(format!(
                    "installation reach resolution for `{identity}` exceeds its published upper bound"
                ));
            }
        }
        Ok(())
    }
}
