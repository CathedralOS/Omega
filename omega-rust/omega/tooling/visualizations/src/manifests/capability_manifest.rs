//! The capability manifest, and the composition and selection variants that differ
//! only in what they are given to resolve against.

use crate::encoding::manifest_values::push_json_string;
use checked_trees::CheckedTrees;
use flow_effects::CapabilityFlowKind;
use symbols::SymbolHandle;

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
    selected: Option<&effects::SelectedProviderPlanFacts>,
) -> String {
    capability_manifest_json_with_composition(program, selected_entry_machine, selected, None)
}

pub fn capability_manifest_json_with_composition(
    program: &CheckedTrees,
    selected_entry_machine: Option<&str>,
    selected: Option<&effects::SelectedProviderPlanFacts>,
    component_progress: Option<&effects::ComponentProgressManifest>,
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
            json.push_str("], \"provider_plan_report_identity\": ");
            push_json_string(
                &mut json,
                &format!("{:#018x}", resolved.provider_plan_report_identity),
            );
        }
        json.push('}');
    }
    json.push_str("],\n  \"may_block\": ");
    json.push_str(if manifest.may_block { "true" } else { "false" });
    json.push_str(",\n  \"component_progress_manifest_report_identity\": ");
    if let Some(report_identity) = manifest.component_progress_manifest_report_identity {
        push_json_string(&mut json, &format!("{report_identity:#018x}"));
    } else {
        json.push_str("null");
    }
    json.push_str(",\n  \"component_progress_status\": ");
    push_json_string(
        &mut json,
        if manifest
            .component_progress_manifest_report_identity
            .is_none()
        {
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
        json.push_str(", \"requirement_owner_package\": ");
        match &demand.requirement_owner_package {
            Some(identity) => push_json_string(&mut json, identity),
            None => json.push_str("null"),
        }
        json.push_str(", \"provider_service\": ");
        push_json_string(&mut json, &demand.provider_service);
        json.push_str(", \"provider_service_package\": ");
        match &demand.provider_service_package {
            Some(identity) => push_json_string(&mut json, identity),
            None => json.push_str("null"),
        }
        json.push_str(", \"provider_plan_report_identity\": ");
        push_json_string(
            &mut json,
            &format!("{:#018x}", demand.provider_plan_report_identity),
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct EntryCapabilityManifest {
    entry_machine: String,
    entry_state: String,
    service_reach: Vec<String>,
    installation_bound_reaches: Vec<InstallationBoundReachManifest>,
    may_suspend: bool,
    may_block: bool,
    component_progress_manifest_report_identity: Option<u64>,
    build_bound_progress_demands: Vec<BuildBoundProgressManifest>,
    capability_flow_counts: [(CapabilityFlowKind, usize); 5],
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BuildBoundProgressManifest {
    provider_service: String,
    provider_service_package: Option<String>,
    provider_plan_report_identity: u64,
    requirement: String,
    requirement_owner_package: Option<String>,
    profile: String,
    subject_projections: Vec<String>,
    establishment_routes: Vec<effects::provider_plan::ServiceProgressEstablishmentRoute>,
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
    provider_plan_report_identity: u64,
    services: Vec<String>,
}

fn entry_capability_manifest(
    program: &CheckedTrees,
    selected_entry_machine: Option<&str>,
    selected: Option<&effects::SelectedProviderPlanFacts>,
    component_progress: Option<&effects::ComponentProgressManifest>,
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
            component_progress_manifest_report_identity: None,
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
            || reach.inferred_transitive == language_semantics::ServiceReachRowTable::EMPTY_ROW,
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
            provider_service_package: demand
                .provider_service_package_identity
                .map(package_identity_hex),
            provider_plan_report_identity: demand.provider_plan_report_identity,
            requirement: demand.requirement_identity.clone(),
            requirement_owner_package: demand
                .requirement_owner_package_identity
                .map(package_identity_hex),
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
        component_progress_manifest_report_identity: component_progress
            .map(effects::ComponentProgressManifest::compatibility_report_identity),
        build_bound_progress_demands,
        capability_flow_counts: capability_flow_counts(program),
    }
}

fn package_identity_hex(identity: semantic_vocabulary::PackageKeyIdentity) -> String {
    identity
        .digest()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn installation_bound_reaches(
    program: &CheckedTrees,
    reach: &checked_trees::MachineServiceReachRows,
    selected: Option<&effects::SelectedProviderPlanFacts>,
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
                    provider_plan_report_identity: resolution.provider_plan_report_identity,
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
mod tests;
