//! Fixture consent follows the accepted program's calls and selected providers.
//! An empty request is proved by that traversal, never by missing custody.

use compiler::CheckedCompilation;
use diagnostics::Diagnostic;
use semantic_vocabulary::{BoundaryMachineId, MachineId, PackageKeyIdentity};

/// The `Console` operations a program resolves against the selected package's
/// provider. Each flag admits the terminal-authority class
/// its methods exercise; an operation the program never resolves is not
/// permitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConsoleUse {
    /// `write`, `write_byte` or `write_line`.
    pub output: bool,
    /// `read_byte` or `read_line`.
    pub input: bool,
}

impl ConsoleUse {
    pub const fn is_empty(self) -> bool {
        !(self.output || self.input)
    }
}

/// The dangerous package-owned services one checked program requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RequiredDangerousServices {
    /// The selected entry demands the standard library's `FilesystemHost`
    /// boundary: its reach names it, directly or through a requirement such as
    /// `FilesystemService::open`.
    pub filesystem: bool,
    /// The selected entry demands the standard library's `TimeHost`
    /// boundary: its reach names it, directly or through a helper such as
    /// `instant_now`.
    pub time: bool,
    /// The operations the program resolves against this package's
    /// selected `Console` provider; `None` when it selected no such provider
    /// or resolves no operation against it.
    pub console: Option<ConsoleUse>,
    /// The program resolves `exit_process` against this package's
    /// selected `ProcessExit` provider.
    pub process_exit: bool,
}

/// One exact call traversal is shared by every package in the fixture.
pub struct DangerousServiceDiscovery {
    packages: Vec<(PackageKeyIdentity, RequiredDangerousServices)>,
}

fn unavailable(reason: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "fixture dangerous-service discovery: {reason}"
    ))]
}

fn reached_boundaries(
    module: &terminal_psi::TerminalModule,
    roots: &[MachineId],
    mut selected_calls: Vec<(BoundaryMachineId, MachineId)>,
) -> Result<Vec<BoundaryMachineId>, Vec<Diagnostic>> {
    selected_calls.sort_unstable();
    selected_calls.dedup();
    let graph = terminal_installation::SynchronousCallGraph::new(module, None)
        .and_then(|graph| graph.with_boundary_calls(selected_calls))
        .map_err(unavailable)?;
    let mut reached = Vec::new();
    for root in roots {
        reached.extend(graph.from_root(*root).map_err(unavailable)?.boundaries);
    }
    reached.sort_unstable();
    reached.dedup();
    Ok(reached)
}

impl DangerousServiceDiscovery {
    pub fn new(checked: &CheckedCompilation) -> Result<Self, Vec<Diagnostic>> {
        let publication = checked.terminal_publication();
        let accepted = publication.accepted()?;
        let module = accepted.module();
        let plans = checked.execution().plans();
        if !plans.assumed_bindings().is_empty() {
            return Err(unavailable(
                "assumed installation reach has no fixture consent projection",
            ));
        }
        let root_reach = plans
            .resolve_root_service_reach(module)
            .map_err(unavailable)?;
        let selected = backend_plan::selected_checked_invocation_rows(
            checked.execution().plans().plans(),
            module,
        )
        .map_err(unavailable)?;
        let mut roots = publication
            .roster()?
            .entries
            .iter()
            .map(|declaration| {
                accepted.machine_of(*declaration).ok_or_else(|| {
                    unavailable("an accepted entry has no retained executable machine")
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        roots.sort_unstable();
        roots.dedup();
        let selected_calls = selected
            .iter()
            .flat_map(|row| &row.candidates)
            .map(|candidate| {
                (
                    candidate.conformance.boundary,
                    candidate.conformance.candidate,
                )
            })
            .collect();
        let reached = reached_boundaries(module, &roots, selected_calls)?;

        let mut operation_services = Vec::with_capacity(reached.len());
        for boundary in &reached {
            let declaration = module
                .boundary_machine(*boundary)
                .ok_or_else(|| unavailable("a reached call lost its boundary declaration"))?;
            operation_services.extend(declaration.owning_service);
        }
        operation_services.sort_unstable();
        operation_services.dedup();
        let mut supplied_boundaries = selected
            .iter()
            .flat_map(|row| &row.candidates)
            .map(|candidate| candidate.conformance.boundary)
            .collect::<Vec<_>>();
        supplied_boundaries.sort_unstable();
        supplied_boundaries.dedup();

        // A foreign contract can require a service without calling one of its
        // methods here. Do not mistake that opaque reach for an empty request.
        // Its consent needs a separate exact operation association.
        if root_reach
            .iter()
            .any(|service| operation_services.binary_search(service).is_err())
        {
            return Err(unavailable(
                "service reach without a reached operation has no fixture consent projection",
            ));
        }

        // A direct foreign binding has no checked provider row. If it is one
        // of these consent roles, its owner cannot be inferred from an address
        // or a method name. Keep that unsupported admission explicit.
        for declaration in accepted
            .boundary_requirements_declared()
            .map_err(|error| vec![error])?
        {
            if reached.binary_search(&declaration.boundary).is_err()
                || supplied_boundaries
                    .binary_search(&declaration.boundary)
                    .is_ok()
            {
                continue;
            }
            let (schema, _) = accepted
                .source()
                .trait_machine_signature(declaration.requirement)
                .ok_or_else(|| unavailable("a reached requirement lost its owning trait"))?;
            if matches!(
                schema.name.as_str(),
                "FilesystemHost" | "TimeHost" | "Console" | "ProcessExit"
            ) {
                return Err(unavailable(format!(
                    "`{}` has no exact checked provider for fixture consent",
                    declaration.name
                )));
            }
        }
        let mut packages: Vec<(PackageKeyIdentity, RequiredDangerousServices)> =
            Vec::with_capacity(4.min(selected.len()));
        for selected in &selected {
            if !selected.candidates.iter().any(|candidate| {
                reached
                    .binary_search(&candidate.conformance.boundary)
                    .is_ok()
            }) {
                continue;
            }
            let role = selected
                .plan
                .schema
                .identity_text(selected.plan.schema.declaration.trait_name);
            // Match provider_acceptance's consent ownership: package services
            // use the plan origin; toolchain ProcessExit uses its provider.
            let owner = match role {
                "ProcessExit" => selected.plan.provider_type_package_identity,
                "FilesystemHost" | "TimeHost" | "Console" => selected.plan.origin_package_identity,
                _ => continue,
            }
            .ok_or_else(|| unavailable(format!("`{role}` has no package consent owner")))?;
            let position = match packages.binary_search_by_key(&owner, |(package, _)| *package) {
                Ok(position) => position,
                Err(position) => {
                    packages.insert(position, (owner, RequiredDangerousServices::default()));
                    position
                }
            };
            record_method(&mut packages[position].1, role, &selected.row.method)?;
        }
        Ok(Self { packages })
    }

    pub fn for_package(&self, owner: PackageKeyIdentity) -> RequiredDangerousServices {
        match self
            .packages
            .binary_search_by_key(&owner, |(package, _)| *package)
        {
            Ok(position) => self.packages[position].1,
            Err(_) => RequiredDangerousServices::default(),
        }
    }
}

fn record_method(
    required: &mut RequiredDangerousServices,
    role: &str,
    method: &str,
) -> Result<(), Vec<Diagnostic>> {
    match role {
        "FilesystemHost" => required.filesystem = true,
        "TimeHost" => required.time = true,
        "ProcessExit" if method == "exit_process" => required.process_exit = true,
        "Console" => {
            let console = required.console.get_or_insert_with(ConsoleUse::default);
            match method {
                "write" | "write_byte" | "write_line" => console.output = true,
                "read_byte" | "read_line" => console.input = true,
                _ => return Err(unavailable(format!("unknown Console operation `{method}`"))),
            }
        }
        "ProcessExit" => {
            return Err(unavailable(format!(
                "unknown ProcessExit operation `{method}`"
            )));
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_call_custody_is_not_an_empty_service_roster() {
        let module = terminal_psi::TerminalModule::declarations();
        let errors =
            reached_boundaries(&module, &[MachineId::new(9).unwrap()], Vec::new()).unwrap_err();
        assert!(errors[0].message.contains("absent machine"));
    }

    #[test]
    fn console_consent_distinguishes_reading_from_writing() {
        let mut required = RequiredDangerousServices::default();
        record_method(&mut required, "Console", "write_byte").unwrap();
        assert_eq!(
            required.console,
            Some(ConsoleUse {
                output: true,
                input: false
            })
        );
        record_method(&mut required, "Console", "read_line").unwrap();
        assert_eq!(
            required.console,
            Some(ConsoleUse {
                output: true,
                input: true
            })
        );
        assert!(record_method(&mut required, "Console", "unknown").is_err());
    }
}
