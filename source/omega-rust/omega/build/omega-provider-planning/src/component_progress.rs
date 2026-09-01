use omega_effects::provider_plan::ProviderBinding;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

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
    program: &psi_checked_trees::CheckedTrees,
    selected: &omega_effects::SelectedProviderPlanFacts,
    exact_source_root: Option<ExactComponentProgressRoot<'_>>,
    test_entry_name: Option<&str>,
) -> Result<Option<omega_effects::ComponentProgressManifest>, Vec<Diagnostic>> {
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
    program: &psi_checked_trees::CheckedTrees,
    selected: &omega_effects::SelectedProviderPlanFacts,
    entry_machine: SymbolHandle,
    entry_callable_identity: String,
) -> Result<omega_effects::ComponentProgressManifest, Vec<Diagnostic>> {
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
            demands.push(omega_effects::CheckedComponentProgressDemand {
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
                    let adapter = crate::plans::exact_checked_adapter(program, plan, row)
                        .map_err(|diagnostic| vec![diagnostic])?;
                    queue.push(adapter.symbol);
                }
            }
        }
    }

    omega_effects::ComponentProgressManifest::bind(entry_callable_identity, selected, demands)
        .map_err(|message| vec![Diagnostic::error(message)])
}

/// TPR6 fail-closed admission seam. Checked lowering and the component
/// manifest preserve exact provider-receiver demands, but a selected provider
/// plan is not itself an establishment receipt. Native/final composition must
/// stop here until the installation occurrence and admitted receipt discharge
/// each exact row.
pub fn reject_undischarged_build_bound_progress(
    manifest: Option<&omega_effects::ComponentProgressManifest>,
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

fn owning_machine<'a>(
    program: &'a psi_checked_trees::CheckedTrees,
    symbol: SymbolHandle,
) -> Option<&'a psi_typed_trees::machine::Machine> {
    program.machines().iter().find(|machine| {
        machine.symbol == symbol
            || program
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == symbol)
    })
}

fn exact_boundary_requirement(
    program: &psi_checked_trees::CheckedTrees,
    symbol: SymbolHandle,
) -> Option<(
    Option<psi_core::PackageKeyIdentity>,
    String,
    Option<psi_core::PackageKeyIdentity>,
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
                        owner.name.as_str().to_owned(),
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
    program: &psi_checked_trees::CheckedTrees,
    symbol: SymbolHandle,
) -> Option<String> {
    let machine = owning_machine(program, symbol)?;
    program
        .normalized_machine_overload_identity(machine)
        .map(|identity| identity.identity())
}

#[cfg(test)]
mod tests {
    use super::{
        ExactComponentProgressRoot, build_selected_component_progress_manifest,
        reject_undischarged_build_bound_progress,
    };
    use omega_effects::provider_plan::{
        ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod,
        ServiceProgressEstablishmentRoute, ServiceProgressEstablishmentRouteKind,
        ServiceProgressPremise, ServiceProgressSubject, ServiceSchema,
    };
    use omega_effects::{
        CheckedComponentProgressDemand, ComponentProgressManifest, SelectedProviderPlanFacts,
    };
    use psi_checked_trees::CheckedTrees;
    use psi_symbols::SymbolHandle;

    fn empty_selected() -> SelectedProviderPlanFacts {
        SelectedProviderPlanFacts::from_selected_plans(Vec::new())
            .expect("empty provider selection")
    }

    fn push_machine(
        program: &mut CheckedTrees,
        name: &str,
        machine_index: u32,
        with_entry: bool,
    ) -> SymbolHandle {
        let machine_symbol = SymbolHandle::from_arena_index(machine_index);
        let mut machine = psi_checked_trees::machine::Machine {
            symbol: machine_symbol,
            name: psi_checked_trees::name::Identifier::generated(name),
            ..Default::default()
        };
        if with_entry {
            program.typed.push_machine_state(
                &mut machine,
                psi_checked_trees::state::State {
                    symbol: SymbolHandle::from_arena_index(machine_index + 1),
                    name: psi_checked_trees::name::Identifier::generated("entry"),
                    ..Default::default()
                },
            );
        }
        program.typed.push_machine(machine);
        machine_symbol
    }

    fn selected() -> SelectedProviderPlanFacts {
        let plan = ProviderPlan {
            name: "scheduler".into(),
            provider_type: "SchedulerProvider".into(),
            provider_type_package_identity: None,
            target: "test".into(),
            schema: ServiceSchema {
                trait_name: "Scheduler".into(),
                trait_package_identity: None,
                methods: vec![ServiceMethod {
                    name: "wait".into(),
                    requirement_owner: "Scheduler".into(),
                    requirement_owner_package_identity: None,
                    requirement_identity: "Scheduler::wait#exact".into(),
                    parameter_count: 0,
                    parameter_type_identities: Vec::new(),
                    entry_claims: Vec::new(),
                    has_result: false,
                    result_type_identity: None,
                    result_claims: Vec::new(),
                    service_reach: vec!["Scheduler".into()],
                    synchronous_invocations: Vec::new(),
                    may_suspend: true,
                    may_block: false,
                    terminates_guarantee: true,
                    termination_premises: vec![ServiceProgressPremise {
                        profile: "WeakFair".into(),
                        subject: ServiceProgressSubject::ProviderReceiver,
                        subject_projections: vec!["queue".into()],
                        establishment_routes: vec![ServiceProgressEstablishmentRoute {
                            kind: ServiceProgressEstablishmentRouteKind::BoundaryRequirement,
                            requirement_identity: "SchedulerAdmission::grant#exact".into(),
                        }],
                    }],
                    calling_plan_report_fingerprint: None,
                    calling_plan_commitment: None,
                }],
            },
            rows: vec![ProviderPlanRow {
                method: "wait".into(),
                requirement_identity: "Scheduler::wait#exact".into(),
                requirement_lifetime_partition: Vec::new(),
                binding: ProviderBinding::CheckedAdapter {
                    machine_identity: "SchedulerProvider::wait".into(),
                    machine_package_identity: None,
                },
            }],
            origin_package_identity: None,
            origin_package: "test".into(),
        };
        SelectedProviderPlanFacts::from_selected_plans(vec![plan])
            .expect("selected progress provider")
    }

    fn demand(call_ordinal: usize) -> CheckedComponentProgressDemand {
        CheckedComponentProgressDemand {
            provider_service_identity: "Scheduler".into(),
            provider_service_package_identity: None,
            requirement_identity: "Scheduler::wait#exact".into(),
            requirement_owner_package_identity: None,
            profile_identity: "WeakFair".into(),
            subject_projections: vec!["queue".into()],
            origin_callable_identity: "Application::run#exact".into(),
            origin_state_identity: "Application::run".into(),
            statement_ordinal: 4,
            call_ordinal,
        }
    }

    #[test]
    fn absent_component_root_builds_no_manifest() {
        assert_eq!(
            build_selected_component_progress_manifest(
                &CheckedTrees::default(),
                &empty_selected(),
                None,
                None,
            ),
            Ok(None),
        );
    }

    #[test]
    fn exact_component_root_retains_its_callable_identity() {
        let mut program = CheckedTrees::default();
        let machine = push_machine(&mut program, "Application::run", 20, true);
        let selected = empty_selected();
        let manifest = build_selected_component_progress_manifest(
            &program,
            &selected,
            Some(ExactComponentProgressRoot::new(
                machine,
                "Application::run#exact",
            )),
            None,
        )
        .expect("exact root should build a manifest")
        .expect("exact root should be present");

        assert_eq!(manifest.entry_callable_identity(), "Application::run#exact");
        assert_eq!(
            manifest.selected_provider_closure_report_identity(),
            selected.compatibility_report_identity()
        );
        assert!(manifest.pending().is_empty());
    }

    #[test]
    fn exact_component_root_outranks_named_harness_fallback() {
        let mut program = CheckedTrees::default();
        let machine = push_machine(&mut program, "Application::run", 20, true);
        let manifest = build_selected_component_progress_manifest(
            &program,
            &empty_selected(),
            Some(ExactComponentProgressRoot::new(
                machine,
                "Application::run#source",
            )),
            Some("missing::HarnessEntry"),
        )
        .expect("exact source root should bypass the harness fallback")
        .expect("exact source root should be present");

        assert_eq!(
            manifest.entry_callable_identity(),
            "Application::run#source"
        );
    }

    #[test]
    fn unique_named_harness_root_uses_its_normalized_callable_identity() {
        let mut program = CheckedTrees::default();
        let machine_symbol = push_machine(&mut program, "Fixture::entry", 20, true);
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == machine_symbol)
            .expect("named harness machine");
        let expected = program
            .normalized_machine_overload_identity(machine)
            .expect("named harness callable identity")
            .identity();

        let manifest = build_selected_component_progress_manifest(
            &program,
            &empty_selected(),
            None,
            Some("Fixture::entry"),
        )
        .expect("unique harness root should build a manifest")
        .expect("unique harness root should be present");

        assert_eq!(manifest.entry_callable_identity(), expected);
    }

    #[test]
    fn named_harness_root_rejects_missing_and_duplicate_machines_exactly() {
        let missing = build_selected_component_progress_manifest(
            &CheckedTrees::default(),
            &empty_selected(),
            None,
            Some("Fixture::entry"),
        )
        .expect_err("missing harness root must reject");
        assert_eq!(
            missing[0].message,
            "selected test entry `Fixture::entry` resolves to 0 checked machines"
        );

        let mut duplicate = CheckedTrees::default();
        push_machine(&mut duplicate, "Fixture::entry", 20, true);
        push_machine(&mut duplicate, "Fixture::entry", 30, true);
        let duplicate = build_selected_component_progress_manifest(
            &duplicate,
            &empty_selected(),
            None,
            Some("Fixture::entry"),
        )
        .expect_err("duplicate harness roots must reject");
        assert_eq!(
            duplicate[0].message,
            "selected test entry `Fixture::entry` resolves to 2 checked machines"
        );
    }

    #[test]
    fn named_harness_root_requires_a_normalized_callable_identity() {
        let mut program = CheckedTrees::default();
        push_machine(&mut program, "Fixture::entry", 20, false);

        let diagnostics = build_selected_component_progress_manifest(
            &program,
            &empty_selected(),
            None,
            Some("Fixture::entry"),
        )
        .expect_err("a bodyless harness root has no normalized callable identity");
        assert_eq!(
            diagnostics[0].message,
            "selected test entry `Fixture::entry` has no normalized callable identity"
        );
    }

    #[test]
    fn absent_progress_manifest_needs_no_discharge() {
        reject_undischarged_build_bound_progress(None)
            .expect("absent progress carries no pending demand");
    }

    #[test]
    fn empty_progress_manifest_needs_no_discharge() {
        let selected = SelectedProviderPlanFacts::from_selected_plans(Vec::new())
            .expect("empty provider selection");
        let manifest =
            ComponentProgressManifest::bind("Application::run#exact".into(), &selected, Vec::new())
                .expect("empty progress manifest");
        reject_undischarged_build_bound_progress(Some(&manifest))
            .expect("empty progress carries no pending demand");
    }

    #[test]
    fn pending_progress_rejects_in_canonical_manifest_order() {
        let manifest = ComponentProgressManifest::bind(
            "Application::run#exact".into(),
            &selected(),
            vec![demand(2), demand(1)],
        )
        .expect("canonical progress manifest");
        let diagnostics = reject_undischarged_build_bound_progress(Some(&manifest))
            .expect_err("pending progress must fail closed before native lowering");
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(
            diagnostics[0].message,
            "final composition cannot discharge build-bound progress demand `Scheduler::wait#exact` requiring profile `WeakFair` at checked call 4:1; the exact installed provider occurrence and admitted establishment receipt must be bound before native lowering",
        );
        assert_eq!(
            diagnostics[1].message,
            "final composition cannot discharge build-bound progress demand `Scheduler::wait#exact` requiring profile `WeakFair` at checked call 4:2; the exact installed provider occurrence and admitted establishment receipt must be bound before native lowering",
        );
    }
}
