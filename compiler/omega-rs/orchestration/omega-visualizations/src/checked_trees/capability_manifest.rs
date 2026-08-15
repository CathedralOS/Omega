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
    crate::phase_diagram::text_report_html(
        "capability_manifest",
        &capability_manifest_text(program, selected_entry_machine),
    )
}

/// Render capabilities for the exact Build-selected entry. `None` reports that
/// no entry was selected; it never discovers one from a source name.
pub fn capability_manifest_json(
    program: &CheckedTrees,
    selected_entry_machine: Option<&str>,
) -> String {
    let manifest = entry_capability_manifest(program, selected_entry_machine);

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
    json.push_str(",\n  \"may_block\": ");
    json.push_str(if manifest.may_block { "true" } else { "false" });
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

fn capability_manifest_text(
    program: &CheckedTrees,
    selected_entry_machine: Option<&str>,
) -> String {
    let manifest = entry_capability_manifest(program, selected_entry_machine);
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
    report.push_str("may suspend:   ");
    report.push_str(if manifest.may_suspend { "yes" } else { "no" });
    report.push('\n');
    report.push_str("may block:     ");
    report.push_str(if manifest.may_block { "yes" } else { "no" });
    report.push('\n');
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
    may_suspend: bool,
    may_block: bool,
    capability_flow_counts: [(CapabilityFlowKind, usize); 5],
}

fn entry_capability_manifest(
    program: &CheckedTrees,
    selected_entry_machine: Option<&str>,
) -> EntryCapabilityManifest {
    let Some((machine_symbol, machine_name, state_name)) =
        entry_machine(program, selected_entry_machine)
    else {
        return EntryCapabilityManifest {
            entry_machine: "<missing>".to_owned(),
            entry_state: "<missing>".to_owned(),
            service_reach: Vec::new(),
            may_suspend: false,
            may_block: false,
            capability_flow_counts: capability_flow_counts(program),
        };
    };

    let contract = program.facts.contract_plans.for_machine(machine_symbol);
    let service_reach = program
        .facts
        .service_reaches
        .for_machine(machine_symbol)
        .map(|reach| reach.inferred_transitive)
        .map(|row| {
            let reaches = &program.facts.service_reaches;
            reaches
                .rows
                .services(row)
                .iter()
                .filter_map(|service| reaches.services.definition(*service))
                .map(|definition| definition.name.clone())
                .collect()
        })
        .unwrap_or_default();

    EntryCapabilityManifest {
        entry_machine: machine_name,
        entry_state: state_name,
        service_reach,
        may_suspend: contract.is_some_and(|contract| contract.suspension.checked_may_suspend),
        may_block: contract.is_some_and(|contract| contract.blocking.checked_may_block),
        capability_flow_counts: capability_flow_counts(program),
    }
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
    use super::{capability_manifest_json, capability_manifest_text};
    use psi_checked_trees::{CheckedTrees, MachineContractPlan, MachineServiceReachRows};
    use psi_language_semantics::{
        BlockingInterface, BlockingPlan, ServiceReachInterface, ServiceReachPlan,
        SuspensionInterface, SuspensionPlan, TerminationGuarantee,
    };
    use psi_symbols::SymbolHandle;
    use psi_typed_trees::machine::Machine;
    use psi_typed_trees::name::Identifier;
    use psi_typed_trees::state::State;

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
                states: Default::default(),
            },
        );

        program
            .facts
            .contract_plans
            .machines
            .push(MachineContractPlan {
                machine: machine_symbol,
                // Deliberately conflicting legacy source: the capability
                // manifest must read reach only from independent facts.
                service_reach: ServiceReachPlan::default(),
                synchronous_invocation: Default::default(),
                suspension: SuspensionPlan {
                    interface: SuspensionInterface::InternalInferred,
                    checked_may_suspend: true,
                },
                blocking: BlockingPlan {
                    interface: BlockingInterface::InternalInferred,
                    checked_may_block: false,
                },
                closed_scalar_values: Default::default(),
                crash: Default::default(),
                termination: psi_language_semantics::MachineTerminationPlan {
                    interface: psi_language_semantics::TerminationInterface::Published(
                        TerminationGuarantee::NoGuarantee,
                    ),
                    ..Default::default()
                },
                fingerprint: 0,
            });

        let json = capability_manifest_json(&program, Some("Application::launch"));
        let text = capability_manifest_text(&program, Some("Application::launch"));

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
}
