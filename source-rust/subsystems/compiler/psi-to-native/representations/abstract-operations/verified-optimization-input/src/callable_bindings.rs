//! Connect boundary calls to the machines selected for this native program.
use super::AdmittedLoweringFacts;
use terminal_semantics::callable_values::{
    CallableBoundaryImplementation, CallableInvocationRoot, CallableInvocationRoots,
};

#[derive(Debug, Default)]
pub struct NativeCallableBindings {
    private_entries: CallableInvocationRoots,
    checked_bodies: Vec<CallableBoundaryImplementation>,
}

impl NativeCallableBindings {
    /// Independent callbacks and interrupt entries start with inputs from the host.
    pub fn private_entries(&self) -> &CallableInvocationRoots {
        &self.private_entries
    }

    /// A call to a checked body passes the values supplied at that call.
    pub fn checked_bodies(&self) -> &[CallableBoundaryImplementation] {
        &self.checked_bodies
    }
}

impl AdmittedLoweringFacts {
    /// Find the private entries each call installs and the selected machine it invokes.
    pub fn callable_bindings(
        &self,
        target: &target_operations::TargetOperationPlan,
    ) -> Result<NativeCallableBindings, String> {
        if target.psi != self.terminal || target.entry != self.entry {
            return Err("callable roots belong to another Terminal program or entry".into());
        }
        Ok(NativeCallableBindings {
            private_entries: self.private_entry_roots()?,
            checked_bodies: checked_boundary_bodies(self.module(), target)?,
        })
    }

    fn private_entry_roots(&self) -> Result<CallableInvocationRoots, String> {
        let module = self.module();
        let installed_helpers =
            provider_correspondence::CallbackProviderDeclarations::installed_helper_roots(module);
        let mut roots = CallableInvocationRoots::new();
        for machine in &module.machines {
            for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
                if !matches!(
                    operation.kind,
                    terminal_psi::OperationKind::BoundaryCall { .. }
                ) {
                    continue;
                }
                let mut entries = Vec::new();
                // Admission owns which helper operations can execute. Ignore an
                // unused proposal; retain the admitted helper's exact declarations.
                if self.installed_interrupts().helpers().iter().any(|helper| {
                    helper.installation_machine() == machine.id
                        && helper.installation() == operation.id
                }) {
                    let index = installed_helpers
                        .binary_search_by_key(&(machine.id, operation.id), |(operation, _)| *operation)
                        .map_err(|_| "admitted interrupt installation lost its exact checked helper declarations")?;
                    entries.extend(
                        installed_helpers[index]
                            .1
                            .as_ref()
                            .map_err(Clone::clone)?
                            .iter()
                            .copied(),
                    );
                }
                entries.extend(
                    self.native_callbacks()
                        .callbacks()
                        .iter()
                        .filter(|callback| {
                            callback.registrar_machine == machine.id
                                && callback.terminal_operation == operation.id
                        })
                        .map(|callback| callback.entry().machine()),
                );
                entries.extend(
                    self.installed_interrupts()
                        .entries()
                        .iter()
                        .filter(|entry| {
                            entry.machine == machine.id && entry.operation == operation.id
                        })
                        .map(|entry| entry.entry().machine()),
                );
                if !entries.is_empty() {
                    entries.sort_unstable();
                    entries.dedup();
                    roots.push(CallableInvocationRoot {
                        caller: machine.id,
                        operation: operation.id,
                        entries,
                    });
                }
            }
        }
        roots.sort_unstable_by_key(|row| (row.caller, row.operation));
        if roots
            .windows(2)
            .any(|pair| (pair[0].caller, pair[0].operation) == (pair[1].caller, pair[1].operation))
        {
            return Err("callable entry roots have ambiguous registrar occurrences".into());
        }
        Ok(roots)
    }
}

fn checked_boundary_bodies(
    module: &terminal_psi::TerminalModule,
    target: &target_operations::TargetOperationPlan,
) -> Result<Vec<CallableBoundaryImplementation>, String> {
    let mut checked_bodies = Vec::new();
    for function in &target.functions {
        for operation in function.graph.operations() {
            let target_operations::TargetUnitOperation::Call {
                origin,
                psi_operation,
                target: callee,
                ..
            } = operation
            else {
                continue;
            };
            let (boundary, selected) = match origin {
                target_operations::NativeCallOrigin::Authored => continue,
                target_operations::NativeCallOrigin::ProgramEntry {
                    boundary, entry, ..
                } => (*boundary, *entry),
                target_operations::NativeCallOrigin::InstalledProvider {
                    boundary,
                    provider,
                    ..
                } => (*boundary, provider.candidate),
            };
            if callee.machine() != Some(selected) {
                return Err("selected native boundary call differs from its admitted body".into());
            }
            let owner = module
                .machine(function.machine)
                .ok_or("selected native boundary caller is absent from the retained module")?;
            let mut sources = owner
                .operations()
                .filter(|source| source.id == *psi_operation);
            let (Some(source), None) = (sources.next(), sources.next()) else {
                return Err("selected native boundary call has no exact source occurrence".into());
            };
            if !matches!(source.kind, terminal_psi::OperationKind::BoundaryCall { boundary: found, .. } if found == boundary)
                || module.machine(selected).is_none()
            {
                return Err(
                    "selected native boundary body does not rejoin its admitted source call".into(),
                );
            }
            checked_bodies.push(CallableBoundaryImplementation {
                boundary,
                machine: selected,
            });
        }
    }
    checked_bodies.sort_unstable();
    checked_bodies.dedup();
    Ok(checked_bodies)
}
