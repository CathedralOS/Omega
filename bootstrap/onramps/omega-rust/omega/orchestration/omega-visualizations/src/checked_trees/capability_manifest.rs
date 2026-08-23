use super::push_json_string;
use psi_checked_trees::CheckedTrees;
use psi_effects::CapabilityFlowKind;
use psi_symbols::SymbolHandle;

/// Render capabilities for the exact Build-selected entry. `None` reports that
/// no entry was selected; it never discovers one from a source name.
pub fn capability_manifest_html(
    program: &CheckedTrees,
    selected_entry_machine: Option<&str>,
) -> String {
    capability_manifest_html_with_composition(program, selected_entry_machine, None, None)
}

pub fn capability_manifest_html_with_selection(
    program: &CheckedTrees,
    selected_entry_machine: Option<&str>,
    selected: Option<&omega_effects::SelectedProviderPlanFacts>,
) -> String {
    capability_manifest_html_with_composition(program, selected_entry_machine, selected, None)
}

pub fn capability_manifest_html_with_composition(
    program: &CheckedTrees,
    selected_entry_machine: Option<&str>,
    selected: Option<&omega_effects::SelectedProviderPlanFacts>,
    component_progress: Option<&omega_effects::ComponentProgressManifest>,
) -> String {
    crate::phase_diagram::text_report_html(
        "capability_manifest",
        &capability_manifest_text_with_composition(
            program,
            selected_entry_machine,
            selected,
            component_progress,
        ),
    )
}

/// Render capabilities for the exact Build-selected entry. `None` reports that
/// no entry was selected; it never discovers one from a source name.
pub fn capability_manifest_json(
    program: &CheckedTrees,
    selected_entry_machine: Option<&str>,
) -> String {
    capability_manifest_json_with_composition(program, selected_entry_machine, None, None)
}

pub fn capability_manifest_json_with_selection(
    program: &CheckedTrees,
    selected_entry_machine: Option<&str>,
    selected: Option<&omega_effects::SelectedProviderPlanFacts>,
) -> String {
    capability_manifest_json_with_composition(program, selected_entry_machine, selected, None)
}

pub fn capability_manifest_json_with_composition(
    program: &CheckedTrees,
    selected_entry_machine: Option<&str>,
    selected: Option<&omega_effects::SelectedProviderPlanFacts>,
    component_progress: Option<&omega_effects::ComponentProgressManifest>,
) -> String {
    let manifest = entry_capability_manifest(
        program,
        selected_entry_machine,
        selected,
        component_progress,
    );

    let mut json = String::new();
    json.push_str("{\n");
    json.push_str("  \"entry_machine\": ");
    push_json_string(&mut json, &manifest.entry_machine);
    json.push_str(",\n  \"entry_state\": ");
    push_json_string(&mut json, &manifest.entry_state);
    json.push_str(",\n  \"service_reach\": [");
    for (index, service) in manifest.service_reach.iter().enumerate() {
        if index > 0 {
            json.push_str(", ");
        }
        push_json_string(&mut json, service);
    }
    json.push_str("],\n  \"may_suspend\": ");
    json.push_str(if manifest.may_suspend {
        "true"
    } else {
        "false"
    });
    json.push_str(",\n  \"installation_bound_reaches\": [");
    for (index, reach) in manifest.installation_bound_reaches.iter().enumerate() {
        if index > 0 {
            json.push_str(", ");
        }
        json.push_str("{\"requirement\": ");
        push_json_string(&mut json, &reach.requirement);
        json.push_str(", \"upper_bound\": [");
        for (service_index, service) in reach.upper_bound.iter().enumerate() {
            if service_index > 0 {
                json.push_str(", ");
            }
            push_json_string(&mut json, service);
        }
        json.push(']');
        if let Some(resolved) = &reach.resolved {
            json.push_str(", \"selected_row\": [");
            for (service_index, service) in resolved.services.iter().enumerate() {
                if service_index > 0 {
                    json.push_str(", ");
                }
                push_json_string(&mut json, service);
            }
            json.push_str("], \"provider_plan_identity\": ");
            push_json_string(
                &mut json,
                &format!("{:#018x}", resolved.provider_plan_identity),
            );
        }
        json.push('}');
    }
    json.push_str("],\n  \"may_block\": ");
    json.push_str(if manifest.may_block { "true" } else { "false" });
    json.push_str(",\n  \"component_progress_manifest_identity\": ");
    if let Some(identity) = manifest.component_progress_manifest_identity {
        push_json_string(&mut json, &format!("{identity:#018x}"));
    } else {
        json.push_str("null");
    }
    json.push_str(",\n  \"component_progress_status\": ");
    push_json_string(
        &mut json,
        if manifest.component_progress_manifest_identity.is_none() {
            "unbound"
        } else if manifest.build_bound_progress_demands.is_empty() {
            "clear"
        } else {
            "pending"
        },
    );
    json.push_str(",\n  \"build_bound_progress_demands\": [");
    for (index, demand) in manifest.build_bound_progress_demands.iter().enumerate() {
        if index > 0 {
            json.push_str(", ");
        }
        json.push_str("{\"requirement\": ");
        push_json_string(&mut json, &demand.requirement);
        json.push_str(", \"provider_service\": ");
        push_json_string(&mut json, &demand.provider_service);
        json.push_str(", \"provider_plan_identity\": ");
        push_json_string(
            &mut json,
            &format!("{:#018x}", demand.provider_plan_identity),
        );
        json.push_str(", \"profile\": ");
        push_json_string(&mut json, &demand.profile);
        json.push_str(", \"subject_projections\": [");
        for (projection_index, projection) in demand.subject_projections.iter().enumerate() {
            if projection_index > 0 {
                json.push_str(", ");
            }
            push_json_string(&mut json, projection);
        }
        json.push_str("], \"authorized_establishment_routes\": [");
        for (route_index, route) in demand.establishment_routes.iter().enumerate() {
            if route_index > 0 {
                json.push_str(", ");
            }
            json.push_str("{\"kind\": ");
            push_json_string(&mut json, route.kind.as_str());
            json.push_str(", \"requirement\": ");
            push_json_string(&mut json, &route.requirement_identity);
            json.push('}');
        }
        json.push_str("], \"origin\": {\"machine\": ");
        push_json_string(&mut json, &demand.origin_machine);
        json.push_str(", \"state\": ");
        push_json_string(&mut json, &demand.origin_state);
        json.push_str(", \"statement\": ");
        json.push_str(&demand.statement_ordinal.to_string());
        json.push_str(", \"call\": ");
        json.push_str(&demand.call_ordinal.to_string());
        json.push_str("}}");
    }
    json.push(']');
    json.push_str(",\n  \"capability_flows\": {");
    for (index, (kind, count)) in manifest.capability_flow_counts.iter().enumerate() {
        if index > 0 {
            json.push_str(", ");
        }
        push_json_string(&mut json, kind.as_str());
        json.push_str(": ");
        json.push_str(&count.to_string());
    }
    json.push_str("}\n}\n");
    json
}

#[cfg(test)]
fn capability_manifest_text(
    program: &CheckedTrees,
    selected_entry_machine: Option<&str>,
    selected: Option<&omega_effects::SelectedProviderPlanFacts>,
) -> String {
    capability_manifest_text_with_composition(program, selected_entry_machine, selected, None)
}

fn capability_manifest_text_with_composition(
    program: &CheckedTrees,
    selected_entry_machine: Option<&str>,
    selected: Option<&omega_effects::SelectedProviderPlanFacts>,
    component_progress: Option<&omega_effects::ComponentProgressManifest>,
) -> String {
    let manifest = entry_capability_manifest(
        program,
        selected_entry_machine,
        selected,
        component_progress,
    );
    let mut report = String::new();

    report.push_str("Executable Capability Manifest\n");
    report.push_str("==============================\n\n");
    report.push_str("entry machine: ");
    report.push_str(&manifest.entry_machine);
    report.push('\n');
    report.push_str("entry state:   ");
    report.push_str(&manifest.entry_state);
    report.push('\n');
    report.push_str("service reach: ");
    if manifest.service_reach.is_empty() {
        report.push_str("<none>");
    } else {
        report.push_str(&manifest.service_reach.join(" + "));
    }
    report.push('\n');
    report.push_str("installation-bound reach: ");
    if manifest.installation_bound_reaches.is_empty() {
        report.push_str("<none>\n");
    } else {
        report.push('\n');
        for reach in &manifest.installation_bound_reaches {
            report.push_str("  ");
            report.push_str(&reach.requirement);
            report.push_str(" <= ");
            report.push_str(&reach.upper_bound.join(" + "));
            if let Some(resolved) = &reach.resolved {
                report.push_str(" => ");
                if resolved.services.is_empty() {
                    report.push_str("<empty>");
                } else {
                    report.push_str(&resolved.services.join(" + "));
                }
            }
            report.push('\n');
        }
    }
    report.push_str("may suspend:   ");
    report.push_str(if manifest.may_suspend { "yes" } else { "no" });
    report.push('\n');
    report.push_str("may block:     ");
    report.push_str(if manifest.may_block { "yes" } else { "no" });
    report.push('\n');
    report.push_str("component progress: ");
    match manifest.component_progress_manifest_identity {
        Some(identity) if manifest.build_bound_progress_demands.is_empty() => {
            report.push_str(&format!("clear ({identity:#018x})\n"));
        }
        Some(identity) => report.push_str(&format!("pending ({identity:#018x})\n")),
        None => report.push_str("unbound\n"),
    }
    report.push_str("build-bound progress: ");
    if manifest.build_bound_progress_demands.is_empty() {
        report.push_str("<none>\n");
    } else {
        report.push('\n');
        for demand in &manifest.build_bound_progress_demands {
            report.push_str("  ");
            report.push_str(&demand.provider_service);
            report.push_str("@");
            report.push_str(&format!("{:#018x}", demand.provider_plan_identity));
            report.push_str(" / ");
            report.push_str(&demand.requirement);
            report.push_str(" requires ");
            report.push_str(&demand.profile);
            if !demand.subject_projections.is_empty() {
                report.push('.');
                report.push_str(&demand.subject_projections.join("."));
            }
            report.push_str(" established by [");
            for (index, route) in demand.establishment_routes.iter().enumerate() {
                if index > 0 {
                    report.push_str(", ");
                }
                report.push_str(route.kind.as_str());
                report.push(':');
                report.push_str(&route.requirement_identity);
            }
            report.push(']');
            report.push_str(" at ");
            report.push_str(&demand.origin_machine);
            report.push_str("::");
            report.push_str(&demand.origin_state);
            report.push(':');
            report.push_str(&demand.statement_ordinal.to_string());
            report.push(':');
            report.push_str(&demand.call_ordinal.to_string());
            report.push('\n');
        }
    }
    report.push_str("\nCapability Flow Counts\n");
    report.push_str("----------------------\n");
    for (kind, count) in manifest.capability_flow_counts {
        report.push_str(kind.as_str());
        report.push_str(": ");
        report.push_str(&count.to_string());
        report.push('\n');
    }

    report
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EntryCapabilityManifest {
    entry_machine: String,
    entry_state: String,
    service_reach: Vec<String>,
    installation_bound_reaches: Vec<InstallationBoundReachManifest>,
    may_suspend: bool,
    may_block: bool,
    component_progress_manifest_identity: Option<u64>,
    build_bound_progress_demands: Vec<BuildBoundProgressManifest>,
    capability_flow_counts: [(CapabilityFlowKind, usize); 5],
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BuildBoundProgressManifest {
    provider_service: String,
    provider_plan_identity: u64,
    requirement: String,
    profile: String,
    subject_projections: Vec<String>,
    establishment_routes: Vec<omega_effects::provider_plan::ServiceProgressEstablishmentRoute>,
    origin_machine: String,
    origin_state: String,
    statement_ordinal: usize,
    call_ordinal: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InstallationBoundReachManifest {
    requirement: String,
    upper_bound: Vec<String>,
    resolved: Option<ResolvedInstallationReachManifest>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedInstallationReachManifest {
    provider_plan_identity: u64,
    services: Vec<String>,
}

fn entry_capability_manifest(
    program: &CheckedTrees,
    selected_entry_machine: Option<&str>,
    selected: Option<&omega_effects::SelectedProviderPlanFacts>,
    component_progress: Option<&omega_effects::ComponentProgressManifest>,
) -> EntryCapabilityManifest {
    let Some((machine_symbol, machine_name, state_name)) =
        entry_machine(program, selected_entry_machine)
    else {
        return EntryCapabilityManifest {
            entry_machine: "<missing>".to_owned(),
            entry_state: "<missing>".to_owned(),
            service_reach: Vec::new(),
            installation_bound_reaches: Vec::new(),
            may_suspend: false,
            may_block: false,
            component_progress_manifest_identity: None,
            build_bound_progress_demands: Vec::new(),
            capability_flow_counts: capability_flow_counts(program),
        };
    };

    let reaches = &program.facts.service_reaches;
    let mut matching_reaches = reaches
        .machines()
        .iter()
        .filter(|reach| reach.machine == machine_symbol);
    let reach = matching_reaches.next().unwrap_or_else(|| {
        panic!("capability manifest invariant: selected entry has no service-reach row")
    });
    assert!(
        matching_reaches.next().is_none(),
        "capability manifest invariant: selected entry has duplicate service-reach rows"
    );
    let service_ids = reaches.rows.services(reach.inferred_transitive);
    assert!(
        !service_ids.is_empty()
            || reach.inferred_transitive == psi_language_semantics::ServiceReachRowTable::EMPTY_ROW,
        "capability manifest invariant: selected entry has a noncanonical inferred service-reach row"
    );
    let service_reach = service_ids
        .iter()
        .map(|service| {
            reaches.services.definition(*service).unwrap_or_else(|| {
                panic!(
                    "capability manifest invariant: selected entry service-reach row contains an unregistered service"
                )
            })
        })
        .map(|definition| definition.name.clone())
        .collect();
    let installation_bound_reaches = installation_bound_reaches(program, reach, selected);
    let build_bound_progress_demands = component_progress
        .into_iter()
        .flat_map(|manifest| manifest.pending())
        .map(|demand| BuildBoundProgressManifest {
            provider_service: demand.provider_service_identity.clone(),
            provider_plan_identity: demand.provider_plan_identity,
            requirement: demand.requirement_identity.clone(),
            profile: demand.profile_identity.clone(),
            subject_projections: demand.subject_projections.clone(),
            establishment_routes: demand.establishment_routes.clone(),
            origin_machine: demand.origin_callable_identity.clone(),
            origin_state: demand.origin_state_identity.clone(),
            statement_ordinal: demand.statement_ordinal,
            call_ordinal: demand.call_ordinal,
        })
        .collect();

    let mut matching_suspensions = program
        .facts
        .suspensions
        .machines
        .iter()
        .filter(|fact| fact.machine == machine_symbol);
    let suspension = matching_suspensions.next().unwrap_or_else(|| {
        panic!("capability manifest invariant: selected entry has no suspension row")
    });
    assert!(
        matching_suspensions.next().is_none(),
        "capability manifest invariant: selected entry has duplicate suspension rows"
    );

    let mut matching_blocking = program
        .facts
        .blocking
        .machines
        .iter()
        .filter(|fact| fact.machine == machine_symbol);
    let blocking = matching_blocking.next().unwrap_or_else(|| {
        panic!("capability manifest invariant: selected entry has no blocking row")
    });
    assert!(
        matching_blocking.next().is_none(),
        "capability manifest invariant: selected entry has duplicate blocking rows"
    );

    EntryCapabilityManifest {
        entry_machine: machine_name,
        entry_state: state_name,
        service_reach,
        installation_bound_reaches,
        may_suspend: suspension.plan.checked_may_suspend,
        may_block: blocking.plan.checked_may_block,
        component_progress_manifest_identity: component_progress
            .map(omega_effects::ComponentProgressManifest::normalized_identity),
        build_bound_progress_demands,
        capability_flow_counts: capability_flow_counts(program),
    }
}

fn installation_bound_reaches(
    program: &CheckedTrees,
    reach: &psi_checked_trees::MachineServiceReachRows,
    selected: Option<&omega_effects::SelectedProviderPlanFacts>,
) -> Vec<InstallationBoundReachManifest> {
    let mut rows = reach
        .unresolved_installation_reaches
        .iter()
        .map(|dependency| {
            let matches = program
                .typed
                .traits()
                .iter()
                .flat_map(|owner| {
                    program
                        .typed
                        .trait_machine_signatures(owner)
                        .iter()
                        .filter(move |requirement| requirement.symbol == dependency.requirement)
                        .map(move |requirement| (owner, requirement))
                })
                .collect::<Vec<_>>();
            let [(owner, requirement)] = matches.as_slice() else {
                panic!(
                    "capability manifest invariant: installation-bound reach requirement resolves to {} typed declarations",
                    matches.len()
                );
            };
            let requirement = program
                .typed
                .normalized_trait_requirement_overload_identity(owner, requirement)
                .identity();
            let upper_bound = program
                .facts
                .service_reaches
                .rows
                .services(dependency.upper_bound)
                .iter()
                .map(|service| {
                    program
                        .facts
                        .service_reaches
                        .services
                        .definition(*service)
                        .unwrap_or_else(|| {
                            panic!(
                                "capability manifest invariant: installation-bound reach contains an unregistered service"
                            )
                        })
                        .name
                        .clone()
                })
                .collect::<Vec<_>>();
            let resolved = selected.map(|selected| {
                let resolution = selected
                    .installation_reach_resolution(&requirement)
                    .unwrap_or_else(|| {
                        panic!(
                            "capability manifest invariant: selected provider closure does not resolve installation-bound requirement `{requirement}`"
                        )
                    });
                assert_eq!(
                    resolution.upper_bound, upper_bound,
                    "capability manifest invariant: selected installation reach bound drifted from checked entry"
                );
                ResolvedInstallationReachManifest {
                    provider_plan_identity: resolution.provider_plan_identity,
                    services: resolution.resolved_row.clone(),
                }
            });
            InstallationBoundReachManifest {
                requirement,
                upper_bound,
                resolved,
            }
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| left.requirement.cmp(&right.requirement));
    assert!(
        !rows
            .windows(2)
            .any(|pair| pair[0].requirement == pair[1].requirement),
        "capability manifest invariant: installation-bound reach requirement is duplicated"
    );
    rows
}

fn capability_flow_counts(program: &CheckedTrees) -> [(CapabilityFlowKind, usize); 5] {
    CapabilityFlowKind::ALL.map(|kind| (kind, program.facts.capabilities.count_by_kind(kind)))
}

fn entry_machine(
    program: &CheckedTrees,
    selected_entry_machine: Option<&str>,
) -> Option<(SymbolHandle, String, String)> {
    selected_entry_machine.and_then(|name| entry_machine_named(program, name))
}

fn entry_machine_named(
    program: &CheckedTrees,
    machine_name: &str,
) -> Option<(SymbolHandle, String, String)> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)?;
    let state = program.machine_states(machine).first()?;
    Some((
        machine.symbol,
        machine.name.as_str().to_owned(),
        state.name.as_str().to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        capability_manifest_json, capability_manifest_json_with_composition,
        capability_manifest_json_with_selection, capability_manifest_text,
    };
    use omega_effects::provider_plan::{
        ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceProgressPremise,
        ServiceProgressSubject, ServiceSchema,
    };
    use psi_checked_trees::{CheckedTrees, MachineContractPlan, MachineServiceReachRows};
    use psi_effects::InstallationReachRequirement;
    use psi_language_semantics::{
        BlockingInterface, BlockingPlan, ServiceReachId, ServiceReachInterface, ServiceReachRowId,
        ServiceReachRowTable, SuspensionInterface, SuspensionPlan, TerminationGuarantee,
    };
    use psi_symbols::SymbolHandle;
    use psi_typed_trees::machine::Machine;
    use psi_typed_trees::name::Identifier;
    use psi_typed_trees::signature::StateSignature;
    use psi_typed_trees::state::State;
    use psi_typed_trees::trait_definition::TraitDefinition;
    use psi_typed_trees::types::TypeReferenceNode;

    fn minimal_manifest_program() -> (CheckedTrees, SymbolHandle) {
        let machine_symbol = SymbolHandle::from_arena_index(30);
        let mut program = CheckedTrees::default();
        let mut machine = Machine {
            symbol: machine_symbol,
            name: Identifier::generated("Application::launch"),
            ..Default::default()
        };
        program.typed.push_machine_state(
            &mut machine,
            State {
                symbol: SymbolHandle::from_arena_index(31),
                name: Identifier::generated("main"),
                ..Default::default()
            },
        );
        program.typed.push_machine(machine);
        program.facts.service_reaches.machines.append_to_span(
            &mut program.facts.service_reaches.root_machines,
            MachineServiceReachRows {
                machine: machine_symbol,
                inferred_transitive: ServiceReachRowTable::EMPTY_ROW,
                ..Default::default()
            },
        );
        program
            .facts
            .suspensions
            .machines
            .push(psi_checked_trees::MachineSuspensionFact {
                machine: machine_symbol,
                plan: SuspensionPlan {
                    interface: SuspensionInterface::InternalInferred,
                    checked_may_suspend: false,
                },
            });
        program
            .facts
            .blocking
            .machines
            .push(psi_checked_trees::MachineBlockingFact {
                machine: machine_symbol,
                plan: BlockingPlan {
                    interface: BlockingInterface::InternalInferred,
                    checked_may_block: false,
                },
            });
        (program, machine_symbol)
    }

    fn manifest_panic(program: &CheckedTrees) -> String {
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            capability_manifest_json(program, Some("Application::launch"))
        }))
        .expect_err("invalid manifest facts must fail closed");
        panic
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| {
                panic
                    .downcast_ref::<&str>()
                    .map(|message| (*message).to_owned())
            })
            .expect("invariant panic has a string diagnostic")
    }

    fn selected_manifest_panic(
        program: &CheckedTrees,
        selected: &omega_effects::SelectedProviderPlanFacts,
    ) -> String {
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            capability_manifest_json_with_selection(
                program,
                Some("Application::launch"),
                Some(selected),
            )
        }))
        .expect_err("invalid selected manifest facts must fail closed");
        panic
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| {
                panic
                    .downcast_ref::<&str>()
                    .map(|message| (*message).to_owned())
            })
            .expect("invariant panic has a string diagnostic")
    }

    #[test]
    fn executable_manifest_uses_normalized_split_behavior_axes() {
        let machine_symbol = SymbolHandle::from_arena_index(10);
        let state_symbol = SymbolHandle::from_arena_index(11);
        let mut program = CheckedTrees::default();

        let mut machine = Machine {
            symbol: machine_symbol,
            name: Identifier::generated("Application::launch"),
            ..Default::default()
        };
        program.typed.push_machine_state(
            &mut machine,
            State {
                symbol: state_symbol,
                name: Identifier::generated("main"),
                ..Default::default()
            },
        );
        program.typed.push_machine(machine);
        let mut fallback = Machine {
            symbol: SymbolHandle::from_arena_index(12),
            name: Identifier::generated("Main::main"),
            ..Default::default()
        };
        program.typed.push_machine_state(
            &mut fallback,
            State {
                symbol: SymbolHandle::from_arena_index(13),
                name: Identifier::generated("main"),
                ..Default::default()
            },
        );
        program.typed.push_machine(fallback);

        let services = &mut program.facts.service_reaches;
        let machine_control = services
            .services
            .intern(SymbolHandle::from_arena_index(20), "MachineControl");
        let port_io = services
            .services
            .intern(SymbolHandle::from_arena_index(21), "PortIo");
        let service_row = services.rows.intern(vec![machine_control, port_io]);
        services.machines.append_to_span(
            &mut services.root_machines,
            MachineServiceReachRows {
                machine: machine_symbol,
                interface: ServiceReachInterface::InternalInferred,
                published_ceiling: psi_language_semantics::ServiceReachRowTable::EMPTY_ROW,
                inferred_direct: service_row,
                inferred_transitive: service_row,
                effective: service_row,
                concrete_effective: service_row,
                unresolved_installation_reaches: Vec::new(),
                states: Default::default(),
            },
        );

        program
            .facts
            .suspensions
            .machines
            .push(psi_checked_trees::MachineSuspensionFact {
                machine: machine_symbol,
                plan: SuspensionPlan {
                    interface: SuspensionInterface::InternalInferred,
                    checked_may_suspend: true,
                },
            });
        program
            .facts
            .blocking
            .machines
            .push(psi_checked_trees::MachineBlockingFact {
                machine: machine_symbol,
                plan: BlockingPlan {
                    interface: BlockingInterface::InternalInferred,
                    checked_may_block: false,
                },
            });
        program
            .facts
            .termination
            .machines
            .push(psi_checked_trees::MachineTerminationFact {
                machine: machine_symbol,
                plan: psi_language_semantics::MachineTerminationPlan {
                    interface: psi_language_semantics::TerminationInterface::Published(
                        TerminationGuarantee::NoGuarantee,
                    ),
                    ..Default::default()
                },
            });
        program
            .facts
            .contract_plans
            .machines
            .push(MachineContractPlan {
                machine: machine_symbol,
                closed_scalar_values: Default::default(),
                crash: Default::default(),
                fingerprint: 0,
            });

        let json = capability_manifest_json(&program, Some("Application::launch"));
        let text = capability_manifest_text(&program, Some("Application::launch"), None);

        assert!(json.contains("\"entry_machine\": \"Application::launch\""));
        assert!(json.contains("\"service_reach\": [\"MachineControl\", \"PortIo\"]"));
        assert!(json.contains("\"may_suspend\": true"));
        assert!(json.contains("\"may_block\": false"));
        assert!(!json.contains("\"effect_bits\""));
        assert!(!json.contains("\"effects\""));
        assert!(text.contains("service reach: MachineControl + PortIo"));
        assert!(text.contains("may suspend:   yes"));
        assert!(text.contains("may block:     no"));
        assert!(!text.contains("effects:"));

        let missing = capability_manifest_json(&program, None);
        assert!(missing.contains("\"entry_machine\": \"<missing>\""));
        assert!(missing.contains("\"entry_state\": \"<missing>\""));
        assert!(!missing.contains("\"entry_machine\": \"Main::main\""));
    }

    #[test]
    fn executable_manifest_preserves_explicit_empty_and_negative_axes() {
        let (program, _) = minimal_manifest_program();
        let json = capability_manifest_json(&program, Some("Application::launch"));

        assert!(json.contains("\"service_reach\": []"));
        assert!(json.contains("\"may_suspend\": false"));
        assert!(json.contains("\"may_block\": false"));
    }

    #[test]
    fn executable_manifest_renders_canonical_build_bound_progress_rows() {
        let (program, _) = minimal_manifest_program();
        let provider = ProviderPlan {
            name: "scheduler".into(),
            provider_type: "SchedulerProvider".into(),
            target: "test".into(),
            schema: ServiceSchema {
                trait_name: "Scheduler".into(),
                methods: vec![ServiceMethod {
                    name: "wait".into(),
                    requirement_owner: "Scheduler".into(),
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
                        subject_projections: Vec::new(),
                        establishment_routes: vec![
                            omega_effects::provider_plan::ServiceProgressEstablishmentRoute {
                                kind: omega_effects::provider_plan::ServiceProgressEstablishmentRouteKind::BoundaryRequirement,
                                requirement_identity: "SchedulerAdmission::grant#exact".into(),
                            },
                        ],
                    }],
                    calling_plan_fingerprint: None,
                }],
            },
            rows: vec![ProviderPlanRow {
                method: "wait".into(),
                requirement_identity: "Scheduler::wait#exact".into(),
                binding: ProviderBinding::CheckedAdapter {
                    machine: "SchedulerProvider::wait".into(),
                },
            }],
            origin_package: "test".into(),
        };
        let selected = omega_effects::SelectedProviderPlanFacts::from_selection(
            &[provider],
            &["scheduler".into()],
        )
        .expect("selected provider");
        let component = omega_effects::ComponentProgressManifest::bind(
            "Application::launch#exact".into(),
            &selected,
            vec![omega_effects::CheckedComponentProgressDemand {
                provider_service_identity: "Scheduler".into(),
                requirement_identity: "Scheduler::wait#exact".into(),
                profile_identity: "WeakFair".into(),
                subject_projections: Vec::new(),
                origin_callable_identity: "Application::launch#exact".into(),
                origin_state_identity: "Application::launch::main".into(),
                statement_ordinal: 2,
                call_ordinal: 1,
            }],
        )
        .expect("canonical component manifest");

        let json = capability_manifest_json_with_composition(
            &program,
            Some("Application::launch"),
            Some(&selected),
            Some(&component),
        );
        assert!(json.contains("\"provider_service\": \"Scheduler\""));
        assert!(json.contains("\"requirement\": \"Scheduler::wait#exact\""));
        assert!(json.contains("\"profile\": \"WeakFair\""));
        assert!(json.contains("\"authorized_establishment_routes\""));
        assert!(json.contains("\"requirement\": \"SchedulerAdmission::grant#exact\""));
        assert!(json.contains("\"provider_plan_identity\""));
        assert!(json.contains("\"component_progress_status\": \"pending\""));
        assert!(json.contains("\"component_progress_manifest_identity\""));
    }

    #[test]
    fn executable_manifest_publishes_unresolved_installation_reach_bounds() {
        let (mut program, machine) = minimal_manifest_program();
        let requirement_symbol = SymbolHandle::from_arena_index(41);
        let unit = program
            .typed
            .type_reference_table
            .insert(TypeReferenceNode::Unit);
        let mut controller = TraitDefinition {
            symbol: SymbolHandle::from_arena_index(40),
            is_boundary: true,
            name: Identifier::generated("InterruptCompletion"),
            ..Default::default()
        };
        program.typed.push_trait_machine_signature(
            &mut controller,
            StateSignature {
                symbol: requirement_symbol,
                name: Identifier::generated("complete"),
                return_type: unit,
                ..Default::default()
            },
        );
        program.typed.push_trait_definition(controller);
        let requirement_identity = {
            let owner = program
                .typed
                .traits()
                .iter()
                .find(|owner| owner.symbol == SymbolHandle::from_arena_index(40))
                .expect("controller trait exists");
            let requirement = program
                .typed
                .trait_machine_signatures(owner)
                .first()
                .expect("completion requirement exists");
            program
                .typed
                .normalized_trait_requirement_overload_identity(owner, requirement)
                .identity()
        };

        program.facts.service_reaches = Default::default();
        let reaches = &mut program.facts.service_reaches;
        let machine_control = reaches
            .services
            .intern(SymbolHandle::from_arena_index(42), "MachineControl");
        let port_io = reaches
            .services
            .intern(SymbolHandle::from_arena_index(43), "PortIo");
        let bound = reaches.rows.intern(vec![machine_control, port_io]);
        reaches.machines.append_to_span(
            &mut reaches.root_machines,
            MachineServiceReachRows {
                machine,
                inferred_transitive: bound,
                unresolved_installation_reaches: vec![InstallationReachRequirement {
                    requirement: requirement_symbol,
                    upper_bound: bound,
                }],
                ..Default::default()
            },
        );

        let json = capability_manifest_json(&program, Some("Application::launch"));
        let text = capability_manifest_text(&program, Some("Application::launch"), None);

        assert!(
            json.contains("InterruptCompletion::complete"),
            "manifest omitted exact requirement identity:\n{json}"
        );
        assert!(
            json.contains("\"upper_bound\": [\"MachineControl\", \"PortIo\"]"),
            "manifest omitted installation-bound ceiling:\n{json}"
        );
        assert!(text.contains("installation-bound reach:"));
        assert!(text.contains("InterruptCompletion::complete"));
        assert!(text.contains("<= MachineControl + PortIo"));

        let provider = ProviderPlan {
            name: "pic".into(),
            provider_type: "LegacyPic".into(),
            target: "test-target".into(),
            schema: ServiceSchema {
                trait_name: "InterruptCompletion".into(),
                methods: vec![ServiceMethod {
                    name: "complete".into(),
                    requirement_owner: "InterruptCompletion".into(),
                    requirement_identity: requirement_identity.clone(),
                    parameter_count: 0,
                    parameter_type_identities: Vec::new(),
                    entry_claims: Vec::new(),
                    has_result: false,
                    result_type_identity: None,
                    result_claims: Vec::new(),
                    service_reach: vec!["PortIo".into()],
                    synchronous_invocations: Vec::new(),
                    may_suspend: false,
                    may_block: false,
                    terminates_guarantee: false,
                    termination_premises: Vec::new(),
                    calling_plan_fingerprint: None,
                }],
            },
            rows: vec![ProviderPlanRow {
                method: "complete".into(),
                requirement_identity: requirement_identity.clone(),
                binding: ProviderBinding::CompilerIntrinsic {
                    machine: "LegacyPic::complete".into(),
                },
            }],
            origin_package: "test".into(),
        };
        let provider_identity = provider.identity_fingerprint();
        let selected_without_reach =
            omega_effects::SelectedProviderPlanFacts::from_selection(&[provider], &["pic".into()])
                .expect("provider selection is valid");
        assert!(
            selected_manifest_panic(&program, &selected_without_reach)
                .contains("does not resolve installation-bound requirement")
        );
        let selected = selected_without_reach
            .with_installation_reach_resolutions(vec![omega_effects::InstallationReachResolution {
                requirement_identity,
                provider_plan_identity: provider_identity,
                upper_bound: vec!["MachineControl".into(), "PortIo".into()],
                resolved_row: vec!["PortIo".into()],
            }])
            .expect("installation reach resolves inside its bound");
        let selected_json = capability_manifest_json_with_selection(
            &program,
            Some("Application::launch"),
            Some(&selected),
        );
        assert!(selected_json.contains("\"selected_row\": [\"PortIo\"]"));
        assert!(selected_json.contains(&format!("{provider_identity:#018x}")));
    }

    #[test]
    fn executable_manifest_rejects_unresolved_reach_without_exact_requirement() {
        let (mut program, machine) = minimal_manifest_program();
        let reaches = &mut program.facts.service_reaches;
        reaches.machines = Default::default();
        reaches.root_machines = Default::default();
        reaches.machines.append_to_span(
            &mut reaches.root_machines,
            MachineServiceReachRows {
                machine,
                inferred_transitive: ServiceReachRowTable::EMPTY_ROW,
                unresolved_installation_reaches: vec![InstallationReachRequirement {
                    requirement: SymbolHandle::from_arena_index(99),
                    upper_bound: ServiceReachRowTable::EMPTY_ROW,
                }],
                ..Default::default()
            },
        );

        assert!(manifest_panic(&program).contains("resolves to 0 typed declarations"));
    }

    #[test]
    fn executable_manifest_rejects_missing_and_duplicate_service_reach() {
        let (mut missing, _) = minimal_manifest_program();
        missing.facts.service_reaches = Default::default();
        assert!(manifest_panic(&missing).contains("no service-reach row"));

        let (mut duplicate, machine) = minimal_manifest_program();
        duplicate.facts.service_reaches.machines.append_to_span(
            &mut duplicate.facts.service_reaches.root_machines,
            MachineServiceReachRows {
                machine,
                inferred_transitive: ServiceReachRowTable::EMPTY_ROW,
                ..Default::default()
            },
        );
        assert!(manifest_panic(&duplicate).contains("duplicate service-reach rows"));
    }

    #[test]
    fn executable_manifest_rejects_missing_and_duplicate_suspension() {
        let (mut missing, _) = minimal_manifest_program();
        missing.facts.suspensions.machines.clear();
        assert!(manifest_panic(&missing).contains("no suspension row"));

        let (mut duplicate, machine) = minimal_manifest_program();
        duplicate
            .facts
            .suspensions
            .machines
            .push(psi_checked_trees::MachineSuspensionFact {
                machine,
                plan: SuspensionPlan {
                    interface: SuspensionInterface::PublishedMaySuspend(true),
                    checked_may_suspend: true,
                },
            });
        assert!(manifest_panic(&duplicate).contains("duplicate suspension rows"));
    }

    #[test]
    fn executable_manifest_rejects_missing_and_duplicate_blocking() {
        let (mut missing, _) = minimal_manifest_program();
        missing.facts.blocking.machines.clear();
        assert!(manifest_panic(&missing).contains("no blocking row"));

        let (mut duplicate, machine) = minimal_manifest_program();
        duplicate
            .facts
            .blocking
            .machines
            .push(psi_checked_trees::MachineBlockingFact {
                machine,
                plan: BlockingPlan {
                    interface: BlockingInterface::PublishedMayBlock(true),
                    checked_may_block: true,
                },
            });
        assert!(manifest_panic(&duplicate).contains("duplicate blocking rows"));
    }

    #[test]
    fn executable_manifest_rejects_noncanonical_and_unregistered_service_rows() {
        let (mut noncanonical, machine) = minimal_manifest_program();
        noncanonical.facts.service_reaches = Default::default();
        noncanonical.facts.service_reaches.machines.append_to_span(
            &mut noncanonical.facts.service_reaches.root_machines,
            MachineServiceReachRows {
                machine,
                inferred_transitive: ServiceReachRowId(99),
                ..Default::default()
            },
        );
        assert!(manifest_panic(&noncanonical).contains("noncanonical inferred service-reach row"));

        let (mut unregistered, machine) = minimal_manifest_program();
        unregistered.facts.service_reaches = Default::default();
        let row = unregistered
            .facts
            .service_reaches
            .rows
            .intern(vec![ServiceReachId(99)]);
        unregistered.facts.service_reaches.machines.append_to_span(
            &mut unregistered.facts.service_reaches.root_machines,
            MachineServiceReachRows {
                machine,
                inferred_transitive: row,
                ..Default::default()
            },
        );
        assert!(manifest_panic(&unregistered).contains("unregistered service"));
    }
}
