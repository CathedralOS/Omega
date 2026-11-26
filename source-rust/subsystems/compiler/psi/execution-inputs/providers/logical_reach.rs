//! Follow the published program's calls, including scalar evaluation and chosen provider bodies.
use diagnostics::Diagnostic;
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use terminal_psi::TerminalModule;
use terminal_semantics::callable_values::CallableBoundaryImplementation;

pub fn boundary_call_reach(
    module: &TerminalModule,
    entry: MachineId,
    implementations: &[CallableBoundaryImplementation],
    target: Option<&str>,
) -> Result<terminal_semantics::callable_values::ReachedBoundaryCalls, Diagnostic> {
    let defect = |detail| {
        Diagnostic::error(format!(
            "cannot determine executable boundary supply: {detail}"
        ))
    };
    let graph = terminal_semantics::synchronous_calls::SynchronousCallGraph::new(module, None)
        .and_then(|graph| {
            graph.with_boundary_calls(
                implementations
                    .iter()
                    .map(|row| (row.boundary, row.machine)),
            )
        })
        .map_err(defect)?;
    let scalar_roots = graph
        .scalar_evaluation_roots(&[entry], target)
        .map_err(defect)?;
    let mut roots = vec![entry];
    let mut boundaries = Vec::new();
    let mut reached_machines = Vec::new();
    for root in scalar_roots {
        match root {
            semantic_vocabulary::TargetMachineCallable::Machine(machine) => roots.push(machine),
            semantic_vocabulary::TargetMachineCallable::Boundary(boundary) => {
                boundaries.push(boundary);
                if let Some(row) = implementations.iter().find(|row| row.boundary == boundary) {
                    roots.push(row.machine);
                }
            }
        }
    }
    roots.sort_unstable();
    roots.dedup();
    for root in roots {
        let reached =
            terminal_semantics::callable_values::boundary_call_reach(module, root, implementations)
                .map_err(defect)?;
        boundaries.extend(reached.boundaries);
        reached_machines.extend(reached.reached_machines);
    }
    boundaries.sort_unstable();
    boundaries.dedup();
    reached_machines.sort_unstable();
    reached_machines.dedup();
    Ok(terminal_semantics::callable_values::ReachedBoundaryCalls {
        boundaries,
        reached_machines,
    })
}

/// All actually reached boundaries and their chosen checked implementations.
/// Retained candidates remain available code; only invocations add demand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedLogicalProviderReach {
    selection: terminal_semantics::logical_providers::ClosedLogicalProviders,
    bindings: Vec<CallableBoundaryImplementation>,
}

impl ClosedLogicalProviderReach {
    pub fn into_parts(
        self,
    ) -> (
        terminal_semantics::logical_providers::ClosedLogicalProviders,
        Vec<CallableBoundaryImplementation>,
    ) {
        (self.selection, self.bindings)
    }

    pub fn requirements(&self) -> &[BoundaryMachineId] {
        self.selection.requirements()
    }
    pub fn selection(&self) -> &terminal_semantics::logical_providers::ClosedLogicalProviders {
        &self.selection
    }
    pub fn bindings(&self) -> &[CallableBoundaryImplementation] {
        &self.bindings
    }
}

pub fn close_providers(
    module: &TerminalModule,
    target: &str,
    entry: Option<MachineId>,
    selected_requirements: &[BoundaryMachineId],
) -> Result<ClosedLogicalProviderReach, Vec<Diagnostic>> {
    let program = terminal_codec::terminal_psi_identity(module).map_err(|error| {
        vec![Diagnostic::error(format!(
            "provider closure lost its canonical identity: {error:?}"
        ))]
    })?;
    let mut bindings = Vec::new();
    loop {
        let (mut reached, invoked_machines) = match entry {
            Some(entry) => {
                let reach = boundary_call_reach(module, entry, &bindings, Some(target))
                    .map_err(|error| vec![error])?;
                (reach.boundaries, reach.reached_machines)
            }
            None => (Vec::new(), Vec::new()),
        };
        for binding in &bindings {
            if selected_requirements
                .binary_search(&binding.boundary)
                .is_ok()
                && invoked_machines.binary_search(&binding.machine).is_err()
            {
                reached.extend(
                    boundary_call_reach(module, binding.machine, &bindings, Some(target))
                        .map_err(|error| vec![error])?
                        .boundaries,
                );
            }
        }
        reached.extend(selected_requirements.iter().copied());
        reached.sort_unstable();
        reached.dedup();
        let selected = terminal_semantics::logical_providers::ClosedLogicalProviders::select(
            module, program, target, &reached,
        )
        .map_err(|error| vec![Diagnostic::error(error.diagnostic(module))])?;
        let mut next = Vec::new();
        for provider in selected.providers() {
            // A receiving compilation reaches the independent requirement. Its supplier
            // closes the implementation in the supplier's own artifact.
            if provider.choice.composition_mode
                == semantic_vocabulary::source_selections::CompositionMode::Independent
            {
                continue;
            }
            for row in &provider.rows {
                let terminal_semantics::logical_providers::LogicalRealization::Checked(positions) =
                    &row.realization
                else {
                    // Sealed physical operations have no Omega body to traverse.
                    continue;
                };
                for &position in positions {
                    let candidate = &module.interfaces.provider_candidates[position];
                    next.push(CallableBoundaryImplementation {
                        boundary: candidate.boundary,
                        machine: candidate.candidate,
                    });
                }
            }
        }
        next.sort_unstable();
        next.dedup();
        if next
            .windows(2)
            .any(|pair| pair[0].boundary == pair[1].boundary)
        {
            return Err(vec![Diagnostic::error(
                "chosen provider has competing exact bodies for one boundary application",
            )]);
        }
        if next == bindings {
            return Ok(ClosedLogicalProviderReach {
                selection: selected,
                bindings,
            });
        }
        if bindings
            .iter()
            .any(|previous| next.binary_search(previous).is_err())
        {
            return Err(vec![Diagnostic::error(
                "provider demand changed an already selected exact boundary body",
            )]);
        }
        bindings = next;
    }
}

/// Install the checked bodies the source chose for this interpreter invocation.
/// Physical backings and independently produced components require their own admission.
pub fn admit_interpreted_providers(
    artifact: terminal_codec::TerminalExecutionView<'_>,
) -> Result<crate::AdmittedProviderSelections, Vec<Diagnostic>> {
    use semantic_vocabulary::source_selections::{CompositionMode, ProductTarget};
    use terminal_semantics::logical_providers::LogicalRealization;
    let module = artifact.module();
    let target = ProductTarget::Interpreter.canonical_name();
    let mut selections = Vec::new();
    if module
        .interfaces
        .provider_selections
        .iter()
        .any(|row| row.target == target)
    {
        let Some(semantic_vocabulary::TargetMachineCallable::Machine(entry)) = module.entry else {
            return Err(vec![Diagnostic::error(
                "an interpreted Build requires a checked machine entry",
            )]);
        };
        let closure = close_providers(module, target, Some(entry), &[])?;
        for provider in closure.selection().providers() {
            let name = module
                .identities
                .text(provider.choice.provider_declaration)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "interpreter provider lost its declaration",
                    )]
                })?;
            if provider.choice.composition_mode == CompositionMode::Independent {
                return Err(vec![Diagnostic::error(format!(
                    "interpreter provider `{name}` requires an admitted independent component"
                ))]);
            }
            for row in &provider.rows {
                let LogicalRealization::Checked(positions) = &row.realization else {
                    return Err(vec![Diagnostic::error(format!(
                        "interpreter provider `{name}` has no admitted external backing for `{}`",
                        row.requirement
                    ))]);
                };
                for &position in positions {
                    let candidate = &module.interfaces.provider_candidates[position];
                    let identity = module
                        .identities
                        .text(candidate.provider_identity)
                        .ok_or_else(|| {
                            vec![Diagnostic::error(
                                "interpreter provider lost its checked implementation identity",
                            )]
                        })?;
                    selections.push(crate::ProviderInstallationSelection {
                        boundary: candidate.boundary,
                        candidate: candidate.candidate,
                        provider_declaration: name.to_owned(),
                        provider_identity: identity.to_owned(),
                    });
                }
            }
        }
        selections.sort_unstable();
        selections.dedup();
    }
    crate::admit_provider_execution_installation(artifact, &selections).map_err(|error| {
        vec![Diagnostic::error(format!(
            "interpreter provider installation: {error:?}"
        ))]
    })
}
