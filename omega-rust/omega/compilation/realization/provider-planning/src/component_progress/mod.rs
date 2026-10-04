//! Component progress: the exact progress root of a selected component,
//! its build-selected progress manifest, and the rejection of undischarged
//! build-bound progress.

use diagnostics::Diagnostic;
use effects::provider_plan::ProviderBinding;
use symbols::SymbolHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactComponentProgressRoot<'a> {
    machine: SymbolHandle,
    callable_identity: &'a str,
}

impl<'a> ExactComponentProgressRoot<'a> {
    pub const fn new(machine: SymbolHandle, callable_identity: &'a str) -> Self {
        Self {
            machine,
            callable_identity,
        }
    }
}

/// Resolve the one selected component entry and construct its canonical
/// progress manifest. An exact source-selected root outranks the test-harness
/// name fallback; absence of both means that this checked product has no
/// component root and therefore no manifest.
pub fn build_selected_component_progress_manifest(
    program: &checked_trees::CheckedTrees,
    selected: &effects::SelectedProviderPlanFacts,
    exact_source_root: Option<ExactComponentProgressRoot<'_>>,
    test_entry_name: Option<&str>,
) -> Result<Option<effects::ComponentProgressManifest>, Vec<Diagnostic>> {
    let root = if let Some(root) = exact_source_root {
        root
    } else if let Some(entry_name) = test_entry_name {
        let matches = program
            .machines()
            .iter()
            .filter(|machine| machine.name.as_str() == entry_name)
            .collect::<Vec<_>>();
        let [entry] = matches.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "selected test entry `{entry_name}` resolves to {} checked machines",
                matches.len()
            ))]);
        };
        let callable_identity = program
            .normalized_machine_overload_identity(entry)
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "selected test entry `{entry_name}` has no normalized callable identity"
                ))]
            })?
            .identity();
        return build_component_progress_manifest(
            program,
            selected,
            entry.symbol,
            callable_identity,
        )
        .map(Some);
    } else {
        return Ok(None);
    };

    build_component_progress_manifest(
        program,
        selected,
        root.machine,
        root.callable_identity.to_owned(),
    )
    .map(Some)
}

fn build_component_progress_manifest(
    program: &checked_trees::CheckedTrees,
    selected: &effects::SelectedProviderPlanFacts,
    entry_machine: SymbolHandle,
    entry_callable_identity: String,
) -> Result<effects::ComponentProgressManifest, Vec<Diagnostic>> {
    let mut queue = vec![entry_machine];
    let mut visited = Vec::new();
    let mut demands = Vec::new();

    while let Some(machine_symbol) = queue.pop() {
        if visited.contains(&machine_symbol) {
            continue;
        }
        visited.push(machine_symbol);

        for demand in program
            .facts
            .termination
            .build_bound_for_machine(machine_symbol)
        {
            demands.push(effects::CheckedComponentProgressDemand {
                provider_service_identity: demand.provider_service_identity.clone(),
                provider_service_package_identity: demand.provider_service_package_identity,
                requirement_identity: demand.requirement_identity.clone(),
                requirement_owner_package_identity: demand.requirement_owner_package_identity,
                profile_identity: demand.profile_identity.clone(),
                subject_projections: demand.subject_projections.clone(),
                origin_callable_identity: machine_callable_identity(program, demand.origin.machine)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "checked build-bound progress demand lost its origin machine identity",
                        )]
                    })?,
                origin_state_identity: program.symbols.display_path(demand.origin.state, "::"),
                statement_ordinal: demand.origin.statement_ordinal,
                call_ordinal: demand.origin.call_ordinal,
            });
        }

        for (_, state) in program
            .facts
            .flow
            .control
            .states
            .iter()
            .filter(|(_, state)| state.machine_symbol == machine_symbol)
        {
            for call in program.facts.flow.control.calls.span_or_empty(state.calls) {
                if let Some(target_machine) = owning_machine(program, call.target_symbol) {
                    queue.push(target_machine.symbol);
                    continue;
                }
                let Some((
                    service_package_identity,
                    service,
                    requirement_owner_package_identity,
                    requirement_identity,
                )) = exact_boundary_requirement(program, call.target_symbol)
                else {
                    continue;
                };
                let plans = selected
                    .plans()
                    .iter()
                    .filter(|plan| {
                        plan.schema.trait_name == service
                            && plan.schema.trait_package_identity == service_package_identity
                    })
                    .collect::<Vec<_>>();
                let plan = match plans.as_slice() {
                    [plan] => *plan,
                    [] => {
                        // No selected checked adapter means this is an
                        // opaque/interpreter leaf for progress-closure
                        // expansion. A direct demand still reaches canonical
                        // binding above and will reject if its provider is
                        // genuinely absent.
                        continue;
                    }
                    _ => {
                        return Err(vec![Diagnostic::error(format!(
                            "reachable boundary requirement `{requirement_identity}` resolves to {} selected provider plans for service `{service}`",
                            plans.len()
                        ))]);
                    }
                };
                let rows = plan
                    .rows
                    .iter()
                    .filter(|row| {
                        row.requirement_identity == requirement_identity
                            && plan.schema.methods.iter().any(|method| {
                                method.requirement_identity == row.requirement_identity
                                    && method.requirement_owner_package_identity
                                        == requirement_owner_package_identity
                            })
                    })
                    .collect::<Vec<_>>();
                let row = match rows.as_slice() {
                    [row] => *row,
                    [] => continue,
                    _ => {
                        return Err(vec![Diagnostic::error(format!(
                            "reachable boundary requirement `{requirement_identity}` resolves to {} rows in selected provider plan `{}`",
                            rows.len(),
                            plan.name
                        ))]);
                    }
                };
                if let ProviderBinding::CheckedAdapter { .. } = &row.binding {
                    let adapter = crate::provider_planning::exact_checked_adapter(
                        crate::ProviderPlanningContext::new(
                            program,
                            source::DependencyScope::Product,
                        ),
                        plan,
                        row,
                    )
                    .map_err(|diagnostic| vec![diagnostic])?;
                    queue.push(adapter.symbol);
                }
            }
        }
    }

    effects::ComponentProgressManifest::bind(entry_callable_identity, selected, demands)
        .map_err(|message| vec![Diagnostic::error(message)])
}

/// TPR6 fail-closed admission seam. Checked lowering and the component
/// manifest preserve exact provider-receiver demands, but a selected provider
/// plan is not itself an establishment receipt. Native/final composition must
/// stop here until the installation occurrence and admitted receipt discharge
/// each exact row.
pub fn reject_undischarged_build_bound_progress(
    manifest: Option<&effects::ComponentProgressManifest>,
) -> Result<(), Vec<Diagnostic>> {
    let Some(manifest) = manifest else {
        return Ok(());
    };
    let demands = manifest.pending();
    if demands.is_empty() {
        return Ok(());
    }
    Err(demands
        .iter()
        .map(|demand| {
            Diagnostic::error(format!(
                "final composition cannot discharge build-bound progress demand `{}` requiring profile `{}` at checked call {}:{}; the exact installed provider occurrence and admitted establishment receipt must be bound before native lowering",
                demand.requirement_identity,
                demand.profile_identity,
                demand.statement_ordinal,
                demand.call_ordinal,
            ))
        })
        .collect())
}

fn owning_machine(
    program: &checked_trees::CheckedTrees,
    symbol: SymbolHandle,
) -> Option<&typed_trees::machine::Machine> {
    program.machines().iter().find(|machine| {
        machine.symbol == symbol
            || program
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == symbol)
    })
}

fn exact_boundary_requirement(
    program: &checked_trees::CheckedTrees,
    symbol: SymbolHandle,
) -> Option<(
    Option<semantic_vocabulary::PackageKeyIdentity>,
    String,
    Option<semantic_vocabulary::PackageKeyIdentity>,
    String,
)> {
    program.traits().iter().find_map(|owner| {
        owner.is_boundary.then_some(()).and_then(|_| {
            program
                .trait_machine_signatures(owner)
                .iter()
                .find(|requirement| requirement.symbol == symbol)
                .map(|requirement| {
                    (
                        program.symbols.symbol_package_identity(owner.symbol),
                        program.trait_declaration_path(owner),
                        program.symbols.symbol_package_identity(requirement.symbol),
                        program
                            .normalized_trait_requirement_overload_identity(owner, requirement)
                            .identity(),
                    )
                })
        })
    })
}

fn machine_callable_identity(
    program: &checked_trees::CheckedTrees,
    symbol: SymbolHandle,
) -> Option<String> {
    let machine = owning_machine(program, symbol)?;
    program
        .normalized_machine_overload_identity(machine)
        .map(|identity| identity.identity())
}
