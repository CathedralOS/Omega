use crate::phase_diagram::PhaseDiagramBuilder;
use crate::service_reach::{append_reach_and_operation_lines, service_names};
use omega_checked_trees::{
    BorrowAccessKind, BorrowArgumentAccessFact, BorrowLoanFact, CheckedTrees,
    FlowBorrowActivationFact, FlowBorrowWeakeningFact, FlowBorrowWeakeningReason, FlowCallFact,
    FlowInvalidationSource, FlowStateFact,
};
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::machine::Machine;
use omega_typed_trees::state::State;
use omega_typed_trees::statement::{
    StatementNode, TableTransition, TransitionTargetHandle, TransitionTargetNode,
};

mod capability_manifest;

pub use capability_manifest::{capability_manifest_html, capability_manifest_json};

pub fn checked_trees_html(program: &CheckedTrees) -> String {
    let mut diagram = PhaseDiagramBuilder::new("checked_trees");
    let mut machine_nodes = Vec::new();
    let mut state_nodes = Vec::new();

    for (machine_index, machine) in program.machines().iter().enumerate() {
        let machine_id = diagram.node(
            format!("machine_{machine_index}"),
            machine_label(program, machine),
            "machine",
            machine_index + 1,
        );
        let reach = machine_service_reach(program, machine.symbol);
        diagram.node_service_reaches(
            &machine_id,
            service_names(
                &program.facts.service_reaches.services,
                &program.facts.service_reaches.rows,
                reach.transitive,
            ),
        );
        machine_nodes.push((machine.symbol, machine_id.clone()));

        for state in program.machine_states(machine) {
            let state_id = diagram.node(
                format!("state_{machine_index}_{}", state.symbol.arena_index()),
                state_label(program, machine, state),
                "state_block",
                machine_index + 1,
            );
            if let Some(flow_state) = flow_state_for(program, machine.symbol, state.symbol) {
                diagram.node_service_reaches(
                    &state_id,
                    service_names(
                        &program.facts.service_reaches.services,
                        &program.facts.service_reaches.rows,
                        flow_state.service_reach.transitive,
                    ),
                );
            }
            diagram.containment_edge(&machine_id, &state_id);
            state_nodes.push((state.symbol, state_id));
        }
    }

    for (machine_index, machine) in program.machines().iter().enumerate() {
        for state in program.machine_states(machine) {
            let Some(source_id) = state_id_for_symbol(&state_nodes, state.symbol) else {
                continue;
            };

            append_checked_call_nodes(
                &mut diagram,
                program,
                machine_index,
                machine,
                state,
                source_id,
                &state_nodes,
            );

            for statement in program.statement_table.statements(state.statement_nodes) {
                if let StatementNode::Transition(transition) = statement
                    && let Some(target_id) = transition_target_id(
                        program,
                        program.machine_states(machine),
                        &state_nodes,
                        transition,
                    )
                {
                    diagram.edge(source_id, target_id, "transition_target");
                }
            }
        }
    }

    diagram.finish()
}

/// Public checked qualification-evidence surface. The fact's program point and
/// its establishment origin remain independent, and admitted rows retain their
/// normalized receipt identity when provider admission supplied one.
pub fn qualification_evidence_manifest_json(program: &CheckedTrees) -> String {
    use omega_core::semantics::QualificationEvidenceOrigin;
    use omega_facts::FactPayload;

    let rows = program
        .facts
        .semantic
        .facts
        .iter()
        .filter(|(_, fact)| fact.evidence.origin != QualificationEvidenceOrigin::None)
        .filter_map(|(_, fact)| {
            let domain_label = match fact.payload {
                FactPayload::DomainMembership {
                    domain,
                    domain_symbol,
                    ..
                }
                | FactPayload::ContractDomainMembership {
                    domain,
                    domain_symbol,
                    ..
                } => {
                    if domain_symbol.is_valid() {
                        qualification_symbol_label(program, domain_symbol)
                    } else {
                        program
                            .domain_path_members(domain)
                            .iter()
                            .map(|member| member.as_str())
                            .collect::<Vec<_>>()
                            .join("::")
                    }
                }
                FactPayload::CarryPermission { permission, .. }
                | FactPayload::ContractCarryPermission { permission, .. } => {
                    permission.name().to_owned()
                }
                _ => return None,
            };
            Some((fact, domain_label))
        })
        .collect::<Vec<_>>();

    let mut json = String::from("{\n  \"qualification_evidence\": [");
    for (index, (fact, domain_label)) in rows.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str("\n    {\n      \"subject\": ");
        push_json_string(&mut json, &qualification_subject(program, fact));
        json.push_str(",\n      \"domain\": ");
        push_json_string(&mut json, domain_label);
        json.push_str(",\n      \"origin\": ");
        push_json_string(&mut json, fact.evidence.origin.as_str());
        json.push_str(",\n      \"program_point\": ");
        push_json_string(&mut json, program_point_name(fact.point));
        json.push_str(",\n      \"source\": ");
        if fact.evidence.source_symbol.is_valid() {
            push_json_string(
                &mut json,
                &qualification_symbol_label(program, fact.evidence.source_symbol),
            );
        } else {
            json.push_str("null");
        }
        json.push_str(",\n      \"requirement\": ");
        if fact.evidence.requirement_symbol.is_valid() {
            push_json_string(
                &mut json,
                &qualification_symbol_label(program, fact.evidence.requirement_symbol),
            );
        } else {
            json.push_str("null");
        }
        json.push_str(",\n      \"receipt_identity\": ");
        if fact.evidence.receipt_identity == 0 {
            json.push_str("null");
        } else {
            push_json_string(
                &mut json,
                &format!("0x{:016x}", fact.evidence.receipt_identity),
            );
        }
        json.push_str("\n    }");
    }
    json.push_str("\n  ],\n  \"canonical_qualification_uses\": [");
    for (index, use_fact) in program
        .facts
        .qualifications
        .canonical_uses
        .iter()
        .enumerate()
    {
        if index > 0 {
            json.push(',');
        }
        json.push_str("\n    {\n      \"machine\": ");
        push_json_string(
            &mut json,
            &qualification_symbol_label(program, use_fact.machine),
        );
        json.push_str(",\n      \"state\": ");
        push_json_string(
            &mut json,
            &qualification_symbol_label(program, use_fact.state),
        );
        json.push_str(",\n      \"statement_index\": ");
        json.push_str(&use_fact.statement_index.to_string());
        json.push_str(",\n      \"kind\": ");
        push_json_string(
            &mut json,
            match use_fact.kind {
                omega_checked_trees::CanonicalQualificationUseKind::ImplicitCast => "implicit_cast",
                omega_checked_trees::CanonicalQualificationUseKind::NamedSatisfierCall => {
                    "named_satisfier_call"
                }
            },
        );
        json.push_str(",\n      \"domain\": ");
        push_json_string(
            &mut json,
            &qualification_symbol_label(program, use_fact.domain),
        );
        json.push_str(",\n      \"satisfier\": ");
        push_json_string(
            &mut json,
            &qualification_symbol_label(program, use_fact.satisfier),
        );
        json.push_str("\n    }");
    }
    json.push_str("\n  ]\n}\n");
    json
}

/// Normalized per-state claim outcome maps retained by the checked ownership
/// pass. This is a proof/debug artifact: it exposes the exact output path and
/// input-or-established source used for n-ary conservation without making the
/// presentation spelling part of public contract identity.
pub fn claim_outcome_manifest_json(program: &CheckedTrees) -> String {
    let ownership = &program.facts.flow.ownership;
    let mut json = String::from("{\n  \"claim_outcome_maps\": [");
    for (map_index, (_, map)) in ownership.claim_outcome_maps.iter().enumerate() {
        if map_index > 0 {
            json.push(',');
        }
        json.push_str("\n    {\n      \"machine\": ");
        push_json_string(&mut json, &symbol_label(program, map.machine_symbol));
        json.push_str(",\n      \"state\": ");
        push_json_string(
            &mut json,
            &state_label_from_symbol(program, map.state_symbol),
        );
        json.push_str(",\n      \"entries\": [");
        for (entry_index, entry) in ownership
            .claim_outcome_entries
            .span_or_empty(map.entries)
            .iter()
            .enumerate()
        {
            if entry_index > 0 {
                json.push(',');
            }
            json.push_str("\n        {\n          \"output_path\": ");
            push_claim_path_json(
                &mut json,
                program,
                ownership.segments.span_or_empty(entry.output_segments),
            );
            json.push_str(",\n          \"source\": ");
            push_claim_outcome_source_json(&mut json, program, entry.source);
            json.push_str("\n        }");
        }
        json.push_str("\n      ]\n    }");
    }
    json.push_str("\n  ]\n}\n");
    json
}

fn push_claim_outcome_source_json(
    json: &mut String,
    program: &CheckedTrees,
    source: omega_checked_trees::FlowClaimOutcomeSource,
) {
    match source {
        omega_checked_trees::FlowClaimOutcomeSource::Unknown => {
            json.push_str("{\"kind\": \"unknown\"}");
        }
        omega_checked_trees::FlowClaimOutcomeSource::Input {
            parameter_symbol,
            segments,
        } => {
            json.push_str("{\"kind\": \"input\", \"parameter\": ");
            push_json_string(json, &symbol_label(program, parameter_symbol));
            json.push_str(", \"path\": ");
            push_claim_path_json(
                json,
                program,
                program
                    .facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(segments),
            );
            json.push('}');
        }
        omega_checked_trees::FlowClaimOutcomeSource::Established {
            claim_identity,
            provenance,
        } => {
            json.push_str("{\"kind\": \"established\", \"claim_identity\": ");
            push_claim_identity_json(json, program, claim_identity);
            json.push_str(", \"provenance\": ");
            push_claim_provenance_json(json, program, provenance);
            json.push('}');
        }
    }
}

fn push_claim_path_json(
    json: &mut String,
    program: &CheckedTrees,
    path: &[omega_facts::PlaceSegment],
) {
    json.push('[');
    for (index, segment) in path.iter().enumerate() {
        if index > 0 {
            json.push_str(", ");
        }
        match segment {
            omega_facts::PlaceSegment::Field { symbol } => {
                json.push_str("{\"field\": ");
                push_json_string(json, &symbol_label(program, *symbol));
                json.push('}');
            }
            omega_facts::PlaceSegment::Case { variant } => {
                json.push_str("{\"case\": ");
                push_json_string(json, &symbol_label(program, *variant));
                json.push('}');
            }
            omega_facts::PlaceSegment::FixedIndex { index } => {
                json.push_str("{\"fixed_index\": ");
                json.push_str(&index.to_string());
                json.push('}');
            }
            omega_facts::PlaceSegment::Index { expression } => {
                json.push_str("{\"index\": ");
                push_json_string(json, &program.expression_table.display_name(*expression));
                json.push('}');
            }
        }
    }
    json.push(']');
}

fn push_claim_identity_json(
    json: &mut String,
    program: &CheckedTrees,
    identity: omega_core::semantics::PermissionClaimIdentity,
) {
    match identity {
        omega_core::semantics::PermissionClaimIdentity::Unknown => {
            json.push_str("{\"kind\": \"unknown\"}");
        }
        omega_core::semantics::PermissionClaimIdentity::Established {
            machine_symbol,
            state_symbol,
            source,
            ordinal,
        } => {
            json.push_str("{\"kind\": \"established\", \"machine\": ");
            push_json_string(json, &symbol_label(program, machine_symbol));
            json.push_str(", \"state\": ");
            push_json_string(json, &state_label_from_symbol(program, state_symbol));
            json.push_str(", \"source\": ");
            push_permission_event_source_json(json, program, source);
            json.push_str(", \"ordinal\": ");
            json.push_str(&ordinal.to_string());
            json.push('}');
        }
    }
}

fn push_claim_provenance_json(
    json: &mut String,
    program: &CheckedTrees,
    provenance: omega_core::semantics::PermissionProvenance,
) {
    match provenance {
        omega_core::semantics::PermissionProvenance::Unknown => {
            json.push_str("{\"kind\": \"unknown\"}");
        }
        omega_core::semantics::PermissionProvenance::Established {
            machine_symbol,
            state_symbol,
            source,
        } => {
            json.push_str("{\"kind\": \"established\", \"machine\": ");
            push_json_string(json, &symbol_label(program, machine_symbol));
            json.push_str(", \"state\": ");
            push_json_string(json, &state_label_from_symbol(program, state_symbol));
            json.push_str(", \"source\": ");
            push_permission_event_source_json(json, program, source);
            json.push('}');
        }
    }
}

fn push_permission_event_source_json(
    json: &mut String,
    program: &CheckedTrees,
    source: omega_core::semantics::PermissionEventSource,
) {
    use omega_core::semantics::PermissionEventSource;
    match source {
        PermissionEventSource::StateEntry => json.push_str("{\"kind\": \"state_entry\"}"),
        PermissionEventSource::Statement { statement_index } => {
            json.push_str("{\"kind\": \"statement\", \"statement_index\": ");
            json.push_str(&statement_index.to_string());
            json.push('}');
        }
        PermissionEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => {
            json.push_str("{\"kind\": \"call\", \"statement_index\": ");
            json.push_str(&statement_index.to_string());
            json.push_str(", \"call_ordinal\": ");
            json.push_str(&call_ordinal.to_string());
            json.push_str(", \"target\": ");
            push_json_string(json, &state_label_from_symbol(program, target_symbol));
            json.push('}');
        }
        PermissionEventSource::StateExit => json.push_str("{\"kind\": \"state_exit\"}"),
    }
}

fn qualification_subject(program: &CheckedTrees, fact: &omega_facts::Fact) -> String {
    use omega_facts::{FactPlace, PlaceRoot, PlaceSegment};

    let FactPlace::Place(place) = fact.place else {
        return match fact.place {
            FactPlace::Symbol(symbol) => qualification_symbol_label(program, symbol),
            FactPlace::Expression(expression) => program.expression_table.display_name(expression),
            FactPlace::TypeReference(type_reference) => {
                program.display_type_reference(type_reference)
            }
            FactPlace::Unknown | FactPlace::Place(_) => "<unknown>".to_owned(),
        };
    };
    let place = program.facts.semantic.places.get(place);
    let mut subject = match place.root {
        PlaceRoot::Unknown => "<unknown>".to_owned(),
        PlaceRoot::Symbol(symbol) => qualification_symbol_label(program, symbol),
        PlaceRoot::Expression(expression) => program.expression_table.display_name(expression),
        PlaceRoot::TypeReference(type_reference) => program.display_type_reference(type_reference),
    };
    for segment in program
        .facts
        .semantic
        .place_segments
        .span_or_empty(place.segments)
    {
        match segment {
            PlaceSegment::Field { symbol } => {
                subject.push('.');
                subject.push_str(&qualification_symbol_label(program, *symbol));
            }
            PlaceSegment::Case { variant } => {
                subject.push_str("::");
                subject.push_str(&qualification_symbol_label(program, *variant));
            }
            PlaceSegment::FixedIndex { index } => {
                subject.push('[');
                subject.push_str(&index.to_string());
                subject.push(']');
            }
            PlaceSegment::Index { expression } => {
                subject.push('[');
                subject.push_str(&program.expression_table.display_name(*expression));
                subject.push(']');
            }
        }
    }
    subject
}

fn qualification_symbol_label(program: &CheckedTrees, symbol: SymbolHandle) -> String {
    if !symbol.is_valid() {
        return "<unknown>".to_owned();
    }
    let path = program.symbols.display_path(symbol, "::");
    if path.is_empty() {
        format!("#{}", symbol.arena_index())
    } else {
        path
    }
}

fn program_point_name(point: omega_facts::ProgramPoint) -> &'static str {
    use omega_facts::ProgramPoint;
    match point {
        ProgramPoint::Global => "global",
        ProgramPoint::Definition { .. } => "definition",
        ProgramPoint::Machine { .. } => "machine",
        ProgramPoint::State { .. } => "state",
        ProgramPoint::Statement { .. } => "statement",
        ProgramPoint::Call { .. } => "call",
        ProgramPoint::CallRequires { .. } => "call_requires",
        ProgramPoint::CallEnsures { .. } => "call_ensures",
        ProgramPoint::Exit { .. } => "exit",
    }
}

/// Checked carry-policy artifact. The authored clause is retained only as a
/// diagnostic/publication input; `effective` is the checker-derived policy
/// later liveness, runtime-admission, and model-export consumers must use.
/// Keeping the axes structured avoids making presentation spelling part of
/// artifact identity.
pub fn carry_manifest_json(program: &CheckedTrees) -> String {
    let mut json = String::from("{\n  \"data\": [");
    for (index, fact) in program.facts.carry.data.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        let name = program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == fact.data)
            .map(|definition| definition.name.as_str())
            .unwrap_or("<unknown>");
        json.push_str("\n    {\n      \"type\": ");
        push_json_string(&mut json, name);
        json.push_str(",\n      \"opaque\": ");
        let opaque = program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == fact.data)
            .is_some_and(|definition| {
                definition.supply_mode == omega_core::semantics::DataSupplyMode::BoundaryOpaque
            });
        json.push_str(if opaque { "true" } else { "false" });
        json.push_str(",\n      \"declared\": ");
        if let Some(declared) = fact.declared {
            push_carry_policy_json(&mut json, declared);
        } else {
            json.push_str("null");
        }
        json.push_str(",\n      \"effective\": ");
        push_carry_policy_json(&mut json, fact.effective);
        json.push_str("\n    }");
    }
    json.push_str("\n  ],\n  \"claim_policies\": [");
    for (index, fact) in program.facts.carry.claim_policies.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str("\n    {\n      \"claim_identity\": ");
        push_claim_identity_json(&mut json, program, fact.claim_identity);
        json.push_str(",\n      \"contributing_origins\": ");
        json.push_str(&fact.contributing_origins.to_string());
        json.push_str(",\n      \"effective\": ");
        push_carry_policy_json(&mut json, fact.effective);
        json.push_str("\n    }");
    }
    json.push_str("\n  ],\n  \"safe_point_crossings\": [");
    for (index, fact) in program.facts.carry.suspension_crossings.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str("\n    {\n      \"machine\": ");
        push_json_string(&mut json, carry_machine_name(program, fact.machine));
        json.push_str(",\n      \"state\": ");
        push_json_string(&mut json, carry_state_name(program, fact.state));
        json.push_str(",\n      \"statement_index\": ");
        json.push_str(&fact.statement_index.to_string());
        json.push_str(",\n      \"call_ordinal\": ");
        json.push_str(&fact.call_ordinal.to_string());
        json.push_str(",\n      \"target\": ");
        push_json_string(&mut json, carry_call_target_name(program, fact.target));
        json.push_str(",\n      \"effective\": ");
        push_carry_policy_json(&mut json, fact.effective);
        json.push_str(",\n      \"live_values\": [");
        for (live_index, live) in fact.live_values.iter().enumerate() {
            if live_index > 0 {
                json.push_str(", ");
            }
            json.push_str("{\"type\": ");
            push_json_string(
                &mut json,
                &program.display_type_reference_with_constraints(live.type_reference),
            );
            json.push_str(", \"storage\": ");
            push_json_string(
                &mut json,
                match live.storage {
                    omega_checked_trees::SuspensionCrossingStorage::Persistent => "persistent",
                    omega_checked_trees::SuspensionCrossingStorage::Parameter => "parameter",
                    omega_checked_trees::SuspensionCrossingStorage::Local => "local",
                    omega_checked_trees::SuspensionCrossingStorage::CallArgument => "call_argument",
                },
            );
            json.push_str(", \"effective\": ");
            push_carry_policy_json(&mut json, live.effective);
            json.push('}');
        }
        json.push_str("]\n    }");
    }
    json.push_str("\n  ],\n  \"activation_wide_carry\": [");
    for (index, fact) in program.facts.carry.activation_wide_carry.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        let name = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == fact.machine)
            .map(|machine| machine.name.as_str())
            .unwrap_or("<unknown>");
        json.push_str("\n    {\n      \"machine\": ");
        push_json_string(&mut json, name);
        json.push_str(",\n      \"analysis_complete\": ");
        json.push_str(if fact.analysis_complete {
            "true"
        } else {
            "false"
        });
        json.push_str(",\n      \"subtree_machine_count\": ");
        json.push_str(
            &program
                .facts
                .carry
                .machine_subtree_symbols(fact.machine)
                .len()
                .to_string(),
        );
        json.push_str(",\n      \"effective\": ");
        push_carry_policy_json(&mut json, fact.effective);
        json.push_str(",\n      \"contributing_type_count\": ");
        json.push_str(&fact.contributing_types.len().to_string());
        json.push_str(",\n      \"unnamed_strict_values\": ");
        json.push_str(&fact.unnamed_strict_values.to_string());
        json.push_str("\n    }");
    }
    json.push_str("\n  ]\n}\n");
    json
}

fn carry_machine_name(program: &CheckedTrees, symbol: SymbolHandle) -> &str {
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol)
        .map(|machine| machine.name.as_str())
        .unwrap_or("<unknown>")
}

fn carry_state_name(program: &CheckedTrees, symbol: SymbolHandle) -> &str {
    program
        .machines()
        .iter()
        .find_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == symbol)
                .map(|state| state.name.as_str())
        })
        .unwrap_or("<unknown>")
}

fn carry_call_target_name(program: &CheckedTrees, symbol: SymbolHandle) -> &str {
    if let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol)
    {
        return machine.name.as_str();
    }
    program
        .machines()
        .iter()
        .find_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == symbol)
                .map(|_| machine.name.as_str())
        })
        .unwrap_or("<unknown>")
}

/// Provider-independent task activation demands. Runtime/provider admission
/// consumes these normalized facts; the artifact keeps target/layout and
/// canonical carry derivation inspectable without exposing provider handles.
pub fn task_activation_manifest_json(program: &CheckedTrees) -> String {
    use omega_checked_trees::{TaskStartOperation, machine::Machine};

    fn machine_name<'a>(machines: &'a [Machine], symbol: SymbolHandle) -> &'a str {
        machines
            .iter()
            .find(|machine| machine.symbol == symbol)
            .map(|machine| machine.name.as_str())
            .unwrap_or("<unknown>")
    }
    let mut json = String::from("{\n  \"activations\": [");
    for (index, activation) in program
        .facts
        .contract_plans
        .task_activations
        .iter()
        .enumerate()
    {
        if index > 0 {
            json.push(',');
        }
        let plan = activation.plan.candidate();
        json.push_str("\n    {\n      \"operation\": ");
        push_json_string(
            &mut json,
            match activation.operation {
                TaskStartOperation::Start => "start",
                TaskStartOperation::TryStart => "try_start",
            },
        );
        json.push_str(",\n      \"start_instance\": ");
        push_json_string(
            &mut json,
            machine_name(program.machines(), activation.start_instance),
        );
        json.push_str(",\n      \"target_machine\": ");
        push_json_string(
            &mut json,
            machine_name(program.machines(), activation.target_machine),
        );
        json.push_str(",\n      \"specialization_fingerprint\": \"0x");
        json.push_str(&format!("{:016x}", activation.specialization_fingerprint));
        json.push_str("\",\n      \"activation_plan_id\": \"0x");
        json.push_str(&format!(
            "{:016x}",
            activation.plan.normalized_identity().normalized_identity()
        ));
        json.push_str("\",\n      \"machine_contract_id\": \"0x");
        json.push_str(&format!(
            "{:016x}",
            plan.machine_contract.normalized_identity()
        ));
        json.push_str("\",\n      \"entry_id\": \"0x");
        json.push_str(&format!("{:016x}", plan.entry.normalized_identity()));
        json.push_str("\",\n      \"argument_layout_id\": \"0x");
        json.push_str(&format!(
            "{:016x}",
            plan.argument_layout.normalized_identity()
        ));
        json.push_str("\",\n      \"terminal_outcome_layout_id\": \"0x");
        json.push_str(&format!(
            "{:016x}",
            plan.terminal_outcome_layout.normalized_identity()
        ));
        json.push_str("\",\n      \"calling_plan_id\": \"0x");
        json.push_str(&format!("{:016x}", plan.calling_plan.normalized_identity()));
        json.push_str("\",\n      \"stack_plan\": {\"bytes\": ");
        json.push_str(&plan.stack_plan.bytes.to_string());
        json.push_str(", \"alignment\": ");
        json.push_str(&plan.stack_plan.alignment.to_string());
        json.push_str(", \"representation\": \"0x");
        json.push_str(&format!(
            "{:016x}",
            plan.stack_plan.representation.normalized_identity()
        ));
        json.push_str("\"},\n      \"may_suspend\": ");
        json.push_str(if plan.may_suspend { "true" } else { "false" });
        json.push_str(",\n      \"may_block\": ");
        json.push_str(if plan.may_block { "true" } else { "false" });
        json.push_str(",\n      \"canonical_suspension_crossings\": [");
        for (crossing_index, crossing) in plan.canonical_suspension_crossings.iter().enumerate() {
            if crossing_index > 0 {
                json.push(',');
            }
            json.push_str("{\"identity\": \"0x");
            json.push_str(&format!("{:016x}", crossing.identity.normalized_identity()));
            json.push_str("\", \"suspension_allowed\": ");
            json.push_str(if crossing.suspension_allowed {
                "true"
            } else {
                "false"
            });
            json.push_str(", \"preserve_cpu\": ");
            json.push_str(if crossing.preserve_cpu {
                "true"
            } else {
                "false"
            });
            json.push_str(", \"preserve_host_thread\": ");
            json.push_str(if crossing.preserve_host_thread {
                "true"
            } else {
                "false"
            });
            json.push('}');
        }
        json.push_str("],\n      \"cpu_thread_preservation\": {\"preserve_cpu\": ");
        json.push_str(if plan.carry_obligations.preserve_cpu {
            "true"
        } else {
            "false"
        });
        json.push_str(", \"preserve_host_thread\": ");
        json.push_str(if plan.carry_obligations.preserve_host_thread {
            "true"
        } else {
            "false"
        });
        json.push('}');
        json.push_str(",\n      \"cancellation_required\": ");
        json.push_str(if plan.cancellation_required {
            "true"
        } else {
            "false"
        });
        json.push_str("\n    }");
    }
    json.push_str("\n  ]\n}\n");
    json
}

fn push_carry_policy_json(output: &mut String, policy: omega_core::semantics::CarryPolicy) {
    use omega_core::semantics::{CarryAddress, CarryCpu, CarryHostThread, CarrySuspension};

    output.push_str("{\"suspension\": ");
    push_json_string(
        output,
        match policy.suspension {
            CarrySuspension::Forbidden => "forbidden",
            CarrySuspension::Allowed => "allowed",
        },
    );
    output.push_str(", \"cpu\": ");
    push_json_string(
        output,
        match policy.cpu {
            CarryCpu::Origin => "same",
            CarryCpu::Any => "any",
        },
    );
    output.push_str(", \"thread\": ");
    push_json_string(
        output,
        match policy.host_thread {
            CarryHostThread::Origin => "same",
            CarryHostThread::Any => "any",
        },
    );
    output.push_str(", \"address\": ");
    push_json_string(
        output,
        match policy.address {
            CarryAddress::Stable => "stable",
            CarryAddress::Movable => "movable",
        },
    );
    output.push('}');
}

/// Decision 20/23's externally inspectable machine-contract artifact. The
/// object shape is the firewall: authored interface identity and checked
/// implementation evidence are siblings, never one flattened bag. Consumers
/// pin `contract.fingerprint`; proof/debug tooling may inspect
/// `implementation` without changing that identity.
pub fn machine_contract_manifest_json(program: &CheckedTrees) -> String {
    let mut json = String::from("{\n  \"machines\": [");
    for (index, machine) in program.machines().iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str("\n    {\n      \"machine\": ");
        push_json_string(&mut json, machine.name.as_str());

        json.push_str(",\n      \"contract\": {");
        if let Some(contract) = program.facts.contract_plans.for_machine(machine.symbol) {
            json.push_str("\n        \"fingerprint\": \"0x");
            json.push_str(&format!("{:016x}", contract.fingerprint));
            json.push_str("\",\n        \"supply\": ");
            push_json_string(&mut json, supply_mode_name(contract.supply_mode));
            json.push_str(",\n        \"service_reach\": ");
            push_service_reach_plan_json(&mut json, program, contract.service_reach);
            json.push_str(",\n        \"suspension\": ");
            push_suspension_plan_json(&mut json, contract.suspension);
            json.push_str(",\n        \"blocking\": ");
            push_blocking_plan_json(&mut json, contract.blocking);
            json.push_str(",\n        \"termination\": ");
            push_termination_interface_json(&mut json, &contract.termination);
            json.push_str("\n      }");
        } else {
            json.push_str("}");
        }

        json.push_str(",\n      \"implementation\": {");
        let mut has_implementation_field = false;
        if let Some(contract) = program.facts.contract_plans.for_machine(machine.symbol) {
            json.push_str("\n        \"checked_may_suspend\": ");
            json.push_str(if contract.suspension.checked_may_suspend {
                "true"
            } else {
                "false"
            });
            json.push_str(",\n        \"checked_may_block\": ");
            json.push_str(if contract.blocking.checked_may_block {
                "true"
            } else {
                "false"
            });
            json.push_str(",\n        \"checked_service_reach\": ");
            push_service_row_json(&mut json, program, contract.service_reach.checked_inferred);
            json.push_str(",\n        \"inferred_write_frames\": [");
            for (frame_index, state_frame) in contract.inferred_write_frames.iter().enumerate() {
                if frame_index > 0 {
                    json.push(',');
                }
                let state_name = program
                    .machine_states(machine)
                    .iter()
                    .find(|state| state.symbol == state_frame.state)
                    .map(|state| state.name.as_str())
                    .unwrap_or("<unknown>");
                json.push_str("\n          {\"state\": ");
                push_json_string(&mut json, state_name);
                json.push_str(", \"completeness\": ");
                push_json_string(
                    &mut json,
                    match state_frame.frame.completeness() {
                        omega_facts::WriteFrameCompleteness::Complete => "complete",
                        omega_facts::WriteFrameCompleteness::Opaque => "opaque",
                    },
                );
                json.push_str(", \"fingerprint\": \"0x");
                json.push_str(&format!("{:016x}", state_frame.frame.fingerprint()));
                json.push_str("\", \"paths\": [");
                push_json_strings(&mut json, state_frame.frame.paths());
                json.push_str("]}");
            }
            if !contract.inferred_write_frames.is_empty() {
                json.push('\n');
                json.push_str("        ");
            }
            json.push(']');
            has_implementation_field = true;
        }
        if let Some(fact) = program.facts.termination.for_machine(machine.symbol) {
            if has_implementation_field {
                json.push(',');
            }
            json.push_str("\n        \"checked_termination\": ");
            push_termination_json(&mut json, &fact.checked_summary);
            json.push_str(",\n        \"resolved_ranking_view\": ");
            push_json_string(&mut json, &fact.resolved_view_path);
            has_implementation_field = true;
        }
        if let Some(witness) = machine.termination_plan.implementation_witness.as_ref() {
            if has_implementation_field {
                json.push(',');
            }
            json.push_str("\n        \"ranking_witness\": {\n          \"subjects\": [");
            push_json_strings(&mut json, &witness.subjects);
            json.push_str("],\n          \"view\": ");
            push_json_string(&mut json, &witness.view_path);
            json.push_str(",\n          \"view_arguments\": [");
            push_json_strings(&mut json, &witness.view_arguments);
            json.push(']');
            if let Some(range) = witness.rank_range.as_ref() {
                json.push_str(",\n          \"rank_range\": {\"floor\": ");
                push_json_string(&mut json, &range.floor);
                json.push_str(", \"ceiling\": ");
                push_json_string(&mut json, &range.ceiling);
                json.push_str(", \"ceiling_inclusive\": ");
                json.push_str(if range.ceiling_inclusive {
                    "true"
                } else {
                    "false"
                });
                json.push('}');
            }
            json.push_str("\n        }");
        }
        json.push_str("\n      }\n    }");
    }
    json.push_str("\n  ],\n  \"specializations\": [");
    for (index, specialization) in program.machine_specializations.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        let template = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == specialization.template)
            .map(|machine| machine.name.as_str())
            .unwrap_or("<unknown>");
        let instance = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == specialization.instance)
            .map(|machine| machine.name.as_str())
            .unwrap_or("<unknown>");
        json.push_str("\n    {\n      \"template\": ");
        push_json_string(&mut json, template);
        json.push_str(",\n      \"instance\": ");
        push_json_string(&mut json, instance);
        json.push_str(",\n      \"instance_fingerprint\": \"0x");
        json.push_str(&format!("{:016x}", specialization.fingerprint));
        json.push_str("\",\n      \"template_contract_fingerprint\": \"0x");
        json.push_str(&format!(
            "{:016x}",
            specialization.template_contract_fingerprint
        ));
        json.push_str("\",\n      \"accepted_template_commitment\": ");
        if let Some(commitment) = specialization.accepted_template_commitment.as_deref() {
            push_json_string(&mut json, commitment);
        } else {
            json.push_str("null");
        }
        json.push_str(",\n      \"type_arguments\": [");
        push_json_strings(&mut json, &specialization.type_arguments);
        json.push_str("],\n      \"machine_argument_contract_fingerprints\": [");
        for (identity_index, identity) in specialization
            .machine_argument_contract_fingerprints
            .iter()
            .enumerate()
        {
            if identity_index > 0 {
                json.push_str(", ");
            }
            push_json_string(&mut json, &format!("0x{identity:016x}"));
        }
        json.push_str("]\n    }");
    }
    json.push_str("\n  ]\n}\n");
    json
}

fn supply_mode_name(mode: omega_core::semantics::MachineSupplyMode) -> &'static str {
    use omega_core::semantics::MachineSupplyMode;
    match mode {
        MachineSupplyMode::CheckedBody => "checked_body",
        MachineSupplyMode::Requirement => "requirement",
        MachineSupplyMode::Boundary => "boundary",
        MachineSupplyMode::Accepted => "accepted",
        MachineSupplyMode::ExternalRealization { .. } => "external-realization",
    }
}

fn push_suspension_plan_json(json: &mut String, plan: omega_core::semantics::SuspensionPlan) {
    use omega_core::semantics::SuspensionInterface;
    match plan.interface {
        SuspensionInterface::InternalInferred => {
            json.push_str("{\"interface\": \"internal_inferred\"}");
        }
        SuspensionInterface::PublishedMaySuspend(value) => {
            json.push_str("{\"interface\": \"published_ceiling\", \"may_suspend\": ");
            json.push_str(if value { "true" } else { "false" });
            json.push('}');
        }
    }
}

fn push_service_reach_plan_json(
    json: &mut String,
    program: &CheckedTrees,
    plan: omega_core::semantics::ServiceReachPlan,
) {
    use omega_core::semantics::ServiceReachInterface;
    match plan.interface {
        ServiceReachInterface::InternalInferred => {
            json.push_str("{\"interface\": \"internal_inferred\"}");
        }
        ServiceReachInterface::PublishedCeiling(row) => {
            json.push_str("{\"interface\": \"published_ceiling\", \"services\": ");
            push_service_row_json(json, program, row);
            json.push('}');
        }
    }
}

fn push_service_row_json(
    json: &mut String,
    program: &CheckedTrees,
    row: omega_core::semantics::ServiceReachRowId,
) {
    let reaches = &program.facts.service_reaches;
    json.push('[');
    for (index, service) in reaches.rows.services(row).iter().enumerate() {
        if index > 0 {
            json.push_str(", ");
        }
        let name = reaches
            .services
            .definition(*service)
            .map(|definition| definition.name.as_str())
            .unwrap_or("<unknown-service>");
        push_json_string(json, name);
    }
    json.push(']');
}

fn push_blocking_plan_json(json: &mut String, plan: omega_core::semantics::BlockingPlan) {
    use omega_core::semantics::BlockingInterface;
    match plan.interface {
        BlockingInterface::InternalInferred => {
            json.push_str("{\"interface\": \"internal_inferred\"}");
        }
        BlockingInterface::PublishedMayBlock(value) => {
            json.push_str("{\"interface\": \"published_ceiling\", \"may_block\": ");
            json.push_str(if value { "true" } else { "false" });
            json.push('}');
        }
    }
}

fn push_termination_json(
    json: &mut String,
    guarantee: &omega_core::semantics::TerminationGuarantee,
) {
    use omega_core::semantics::TerminationGuarantee;
    match guarantee {
        TerminationGuarantee::NoGuarantee => json.push_str("{\"kind\": \"no_guarantee\"}"),
        TerminationGuarantee::EventualTerminal { premises } => {
            json.push_str("{\"kind\": \"eventual_terminal\", \"premises\": [");
            for (index, premise) in premises.iter().enumerate() {
                if index > 0 {
                    json.push_str(", ");
                }
                json.push_str(&premise.0.to_string());
            }
            json.push_str("]}");
        }
    }
}

fn push_termination_interface_json(
    json: &mut String,
    interface: &omega_core::semantics::TerminationInterface,
) {
    use omega_core::semantics::TerminationInterface;
    match interface {
        TerminationInterface::InternalDerived => {
            json.push_str("{\"interface\": \"internal_derived\"}");
        }
        TerminationInterface::Published(guarantee) => {
            json.push_str("{\"interface\": \"published\", \"guarantee\": ");
            push_termination_json(json, guarantee);
            json.push('}');
        }
    }
}

fn push_json_strings(json: &mut String, values: &[String]) {
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            json.push_str(", ");
        }
        push_json_string(json, value);
    }
}

fn machine_label(program: &CheckedTrees, machine: &Machine) -> String {
    let attached_data = machine
        .attached_data
        .as_ref()
        .map(|name| name.as_str())
        .unwrap_or("<none>");
    let mut label = format!(
        "machine {}\nattached data: {}\nmachine contracts: {}  trait satisfies: {}",
        machine.name.as_str(),
        attached_data,
        machine.contracts.len(),
        machine.satisfies.len()
    );
    append_reach_and_operation_lines(
        &mut label,
        &program.facts.service_reaches.services,
        &program.facts.service_reaches.rows,
        machine_service_reach(program, machine.symbol),
        machine_operational_summary(program, machine.symbol),
    );
    label
}

fn state_label(program: &CheckedTrees, machine: &Machine, state: &State) -> String {
    let borrow_state = borrow_state_for(program, machine.symbol, state.symbol);
    let flow_state = flow_state_for(program, machine.symbol, state.symbol);

    let writable_root_count = borrow_state
        .map(|borrow| borrow.writable_roots.len())
        .unwrap_or(0);
    let (invalidation_count, mutable_parameter_count, service_reach, operational) =
        if let Some(flow) = flow_state {
            (
                flow.invalidations.len(),
                flow.mutable_parameter_count,
                flow.service_reach,
                flow.operational,
            )
        } else {
            (
                0,
                borrow_state
                    .map(|borrow| borrow.mutable_parameter_count)
                    .unwrap_or(0),
                Default::default(),
                Default::default(),
            )
        };

    let mut label = format!(
        "{}::{} [checked]\nparams: {}  mutable params: {}\nborrow: roots {}\ninvalidations: {}",
        machine.name.as_str(),
        state.name.as_str(),
        program.state_parameters(state).len(),
        mutable_parameter_count,
        writable_root_count,
        invalidation_count,
    );
    append_reach_and_operation_lines(
        &mut label,
        &program.facts.service_reaches.services,
        &program.facts.service_reaches.rows,
        service_reach,
        operational,
    );

    if let Some(flow) = flow_state {
        append_loan_preview(&mut label, program, machine, state, flow.entry_constraints);
        append_activation_preview(&mut label, program, machine, state, flow);
        append_weakening_preview(&mut label, program, machine, state, flow);
        append_statement_preview(&mut label, program, flow);
        append_exit_preview(&mut label, program, flow);
    }

    label
}

fn append_loan_preview(
    label: &mut String,
    program: &CheckedTrees,
    machine: &Machine,
    state: &State,
    constraints: omega_core::arena::HandleSpan<omega_checked_trees::FlowConstraintRef>,
) {
    let loans = program
        .facts
        .flow
        .borrow_loan_constraints(constraints)
        .take(3)
        .collect::<Vec<_>>();
    for loan in loans {
        label.push_str("\n  entry loan: ");
        label.push_str(&borrow_loan_label(
            program,
            machine,
            state,
            program.facts.borrow.loans.get(loan),
        ));
    }
}

fn append_activation_preview(
    label: &mut String,
    program: &CheckedTrees,
    machine: &Machine,
    state: &State,
    flow: &FlowStateFact,
) {
    let activations = program
        .facts
        .flow
        .borrow_lifetimes
        .activations
        .span_or_empty(flow.borrow_activations);
    for activation in activations.iter().take(3) {
        label.push_str("\n  activation: ");
        label.push_str(&borrow_activation_label(
            program, machine, state, activation,
        ));
    }
    if activations.len() > 3 {
        label.push_str("\n  ... ");
        label.push_str(&(activations.len() - 3).to_string());
        label.push_str(" more activations");
    }
}

fn append_weakening_preview(
    label: &mut String,
    program: &CheckedTrees,
    machine: &Machine,
    state: &State,
    flow: &FlowStateFact,
) {
    let weakenings = program
        .facts
        .flow
        .borrow_lifetimes
        .weakenings
        .span_or_empty(flow.borrow_weakenings);
    for weakening in weakenings.iter().take(3) {
        label.push_str("\n  weakening: ");
        label.push_str(&borrow_weakening_label(program, machine, state, weakening));
    }
    if weakenings.len() > 3 {
        label.push_str("\n  ... ");
        label.push_str(&(weakenings.len() - 3).to_string());
        label.push_str(" more weakenings");
    }
}

fn append_statement_preview(label: &mut String, program: &CheckedTrees, flow: &FlowStateFact) {
    let statements = program
        .facts
        .flow
        .control
        .statements
        .span_or_empty(flow.statements);
    for statement in statements.iter().take(6) {
        label.push_str("\n  stmt #");
        label.push_str(&statement.statement_index.to_string());
        label.push_str(": ctx ");
        label.push_str(&statement.entry_semantic_contexts.len().to_string());
        label.push_str(" loans ");
        label.push_str(
            &program
                .facts
                .flow
                .borrow_loan_constraints(statement.entry_constraints)
                .count()
                .to_string(),
        );
    }
    if statements.len() > 6 {
        label.push_str("\n  ... ");
        label.push_str(&(statements.len() - 6).to_string());
        label.push_str(" more statements");
    }
}

fn append_exit_preview(label: &mut String, program: &CheckedTrees, flow: &FlowStateFact) {
    let exits = program.facts.flow.control.exits.span_or_empty(flow.exits);
    for exit in exits.iter().take(3) {
        label.push_str("\n  exit #");
        label.push_str(&exit.statement_index.to_string());
        label.push_str(": ensures ");
        label.push_str(&exit.ensures.len().to_string());
        label.push_str(" ctx ");
        label.push_str(&exit.ensures_contexts.len().to_string());
    }
}

fn append_checked_call_nodes(
    diagram: &mut PhaseDiagramBuilder,
    program: &CheckedTrees,
    machine_index: usize,
    machine: &Machine,
    state: &State,
    source_id: &str,
    state_nodes: &[(SymbolHandle, String)],
) {
    let Some(flow_state) = flow_state_for(program, machine.symbol, state.symbol) else {
        return;
    };

    for call in program
        .facts
        .flow
        .control
        .calls
        .span_or_empty(flow_state.calls)
    {
        let label = checked_call_label(program, machine, state, call);
        let call_id = format!(
            "checked_call_{}_{}_{}_{}",
            machine_index,
            state.symbol.arena_index(),
            call.statement_index,
            call.call_ordinal
        );

        let rendered_id =
            if let Some(target_id) = state_id_for_symbol(state_nodes, call.target_symbol) {
                if target_id == source_id {
                    diagram.node(call_id, label, "external_call", machine_index + 1)
                } else {
                    diagram.scoped_node(
                        call_id,
                        label,
                        "external_call",
                        machine_index + 1,
                        target_id,
                    )
                }
            } else {
                diagram.node(call_id, label, "external_call", machine_index + 1)
            };

        diagram.node_service_reaches(
            &rendered_id,
            service_names(
                &program.facts.service_reaches.services,
                &program.facts.service_reaches.rows,
                call.service_reach.transitive,
            ),
        );
        diagram.edge(source_id, &rendered_id, "call");
        diagram.containment_edge(source_id, &rendered_id);
    }
}

fn checked_call_label(
    program: &CheckedTrees,
    machine: &Machine,
    state: &State,
    call: &FlowCallFact,
) -> String {
    let access_text = borrow_access_summary(program, machine, state, call.accesses);
    let mut label = format!(
        "call {}\nat #{}.{}\nentry: ctx {} constraints {} loans {}\ncontracts: requires {} ensures {}\nborrow: access {} invalidations {}",
        state_label_from_symbol(program, call.target_symbol),
        call.statement_index,
        call.call_ordinal,
        call.entry_semantic_contexts.len(),
        call.entry_constraints.len(),
        program
            .facts
            .flow
            .borrow_loan_constraints(call.entry_constraints)
            .count(),
        call.requires.len(),
        call.ensures.len(),
        access_text,
        call.invalidations.len(),
    );
    append_reach_and_operation_lines(
        &mut label,
        &program.facts.service_reaches.services,
        &program.facts.service_reaches.rows,
        call.service_reach,
        call.operational,
    );
    let acknowledgement = call.operational_acknowledgement;
    let acknowledgement_text = match (
        acknowledgement.acknowledges_suspend,
        acknowledgement.acknowledges_block,
    ) {
        (false, false) => "neither",
        (true, false) => "suspend",
        (false, true) => "block",
        (true, true) => "suspend block",
    };
    let origin = match acknowledgement.origin {
        omega_core::semantics::CallOperationalAcknowledgementOrigin::Source => "source",
        omega_core::semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized => {
            "compiler-synthesized"
        }
    };
    label.push_str(&format!(
        "\nacknowledgement: {acknowledgement_text} ({origin})"
    ));
    label.push_str("\n\ndouble-click to scope target");
    label
}

fn borrow_access_summary(
    program: &CheckedTrees,
    machine: &Machine,
    state: &State,
    accesses: omega_core::arena::HandleSpan<BorrowArgumentAccessFact>,
) -> String {
    let access_facts = program
        .facts
        .borrow
        .argument_accesses
        .span_or_empty(accesses);
    if access_facts.is_empty() {
        return "<none>".to_owned();
    }

    access_facts
        .iter()
        .map(|access| borrow_access_label(program, machine, state, access))
        .collect::<Vec<_>>()
        .join(" | ")
}

fn borrow_access_label(
    program: &CheckedTrees,
    machine: &Machine,
    state: &State,
    access: &BorrowArgumentAccessFact,
) -> String {
    let mut label = symbol_name_for_state(program, machine, state, access.root_symbol);
    for segment in program
        .facts
        .borrow
        .access_segments
        .span_or_empty(access.segments)
    {
        match segment {
            omega_facts::PlaceSegment::Field { symbol } => {
                label.push('.');
                label.push_str(&symbol_name_for_state(program, machine, state, *symbol));
            }
            omega_facts::PlaceSegment::Case { variant } => {
                label.push_str("::");
                label.push_str(&symbol_name_for_state(program, machine, state, *variant));
            }
            omega_facts::PlaceSegment::FixedIndex { index } => {
                label.push('[');
                label.push_str(&index.to_string());
                label.push(']');
            }
            omega_facts::PlaceSegment::Index { expression } => {
                label.push('[');
                label.push_str(&program.expression_table.display_name(*expression));
                label.push(']');
            }
        }
    }
    label.push_str(": ");
    label.push_str(match access.kind {
        BorrowAccessKind::Read => "read",
        BorrowAccessKind::Mutable => "mutable",
    });
    label
}

fn borrow_loan_label(
    program: &CheckedTrees,
    machine: &Machine,
    state: &State,
    loan: &BorrowLoanFact,
) -> String {
    let mut place = symbol_name_for_state(program, machine, state, loan.root_symbol);
    for segment in program
        .facts
        .borrow
        .access_segments
        .span_or_empty(loan.segments)
    {
        match segment {
            omega_facts::PlaceSegment::Field { symbol } => {
                place.push('.');
                place.push_str(&symbol_name_for_state(program, machine, state, *symbol));
            }
            omega_facts::PlaceSegment::Case { variant } => {
                place.push_str("::");
                place.push_str(&symbol_name_for_state(program, machine, state, *variant));
            }
            omega_facts::PlaceSegment::FixedIndex { index } => {
                place.push('[');
                place.push_str(&index.to_string());
                place.push(']');
            }
            omega_facts::PlaceSegment::Index { expression } => {
                place.push('[');
                place.push_str(&program.expression_table.display_name(*expression));
                place.push(']');
            }
        }
    }

    format!(
        "{} -> {} [created {}, last use {}]",
        symbol_name_for_state(program, machine, state, loan.owner_symbol),
        place,
        loan.statement_index,
        loan.last_use_statement_index
    )
}

fn borrow_activation_label(
    program: &CheckedTrees,
    machine: &Machine,
    state: &State,
    activation: &FlowBorrowActivationFact,
) -> String {
    format!(
        "{} -> {}",
        borrow_event_source_label(program, activation.source),
        borrow_loan_label(
            program,
            machine,
            state,
            program.facts.borrow.loans.get(activation.loan),
        ),
    )
}

fn borrow_weakening_label(
    program: &CheckedTrees,
    machine: &Machine,
    state: &State,
    weakening: &FlowBorrowWeakeningFact,
) -> String {
    format!(
        "{} -> {} ({})",
        borrow_event_source_label(program, weakening.source),
        borrow_loan_label(
            program,
            machine,
            state,
            program.facts.borrow.loans.get(weakening.loan),
        ),
        borrow_weakening_reason_label(weakening.reason),
    )
}

fn borrow_event_source_label(program: &CheckedTrees, source: FlowInvalidationSource) -> String {
    match source {
        FlowInvalidationSource::Statement { statement_index } => {
            format!("statement {statement_index}")
        }
        FlowInvalidationSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => format!(
            "call #{}.{} -> {}",
            statement_index,
            call_ordinal,
            state_label_from_symbol(program, target_symbol)
        ),
    }
}

fn borrow_weakening_reason_label(reason: FlowBorrowWeakeningReason) -> &'static str {
    match reason {
        FlowBorrowWeakeningReason::LastUseExpired => "after last use",
        FlowBorrowWeakeningReason::StateExit => "at state exit",
        FlowBorrowWeakeningReason::LocalReassigned => "after local reassignment",
    }
}

fn symbol_name_for_state(
    program: &CheckedTrees,
    machine: &Machine,
    state: &State,
    symbol: SymbolHandle,
) -> String {
    if symbol == machine.symbol {
        return "self".to_owned();
    }

    if let Some(parameter) = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == symbol)
    {
        return parameter.name.as_str().to_owned();
    }

    if let Some(owned) = program
        .machine_owned_data(machine)
        .iter()
        .find(|owned| owned.symbol == symbol)
    {
        return owned.name.as_str().to_owned();
    }

    semantic_symbol_name(program, symbol)
}

fn flow_state_for(
    program: &CheckedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Option<&FlowStateFact> {
    program
        .facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == machine_symbol && state.state_symbol == state_symbol)
                .then_some(state)
        })
}

fn borrow_state_for(
    program: &CheckedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Option<&omega_checked_trees::StateBorrowFact> {
    program.facts.borrow.states.iter().find_map(|(_, state)| {
        (state.machine_symbol == machine_symbol && state.state_symbol == state_symbol)
            .then_some(state)
    })
}

fn machine_service_reach(
    program: &CheckedTrees,
    symbol: SymbolHandle,
) -> omega_core::semantics::ServiceReachSummary {
    program
        .facts
        .service_reaches
        .for_machine(symbol)
        .map(|reach| omega_core::semantics::ServiceReachSummary {
            direct: reach.inferred_direct,
            transitive: reach.inferred_transitive,
        })
        .unwrap_or_default()
}

fn machine_operational_summary(
    program: &CheckedTrees,
    symbol: SymbolHandle,
) -> omega_core::semantics::OperationalMaySummary {
    let mut summary = omega_core::semantics::OperationalMaySummary::default();
    for flow in program
        .facts
        .flow
        .control
        .states
        .iter()
        .map(|(_, state)| state)
        .filter(|state| state.machine_symbol == symbol)
    {
        summary.direct_may_suspend |= flow.operational.direct_may_suspend;
        summary.direct_may_block |= flow.operational.direct_may_block;
    }
    if let Some(machine) = program
        .facts
        .operations
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol)
    {
        summary.transitive_may_suspend = machine.transitive_may_suspend;
        summary.transitive_may_block = machine.transitive_may_block;
    }
    summary
}

fn state_id_for_symbol(
    state_nodes: &[(SymbolHandle, String)],
    symbol: SymbolHandle,
) -> Option<&str> {
    state_nodes
        .iter()
        .find(|(candidate, _)| *candidate == symbol)
        .map(|(_, id)| id.as_str())
}

fn transition_target_id<'states>(
    program: &CheckedTrees,
    states: &'states [State],
    state_nodes: &'states [(SymbolHandle, String)],
    transition: &TableTransition,
) -> Option<&'states str> {
    transition_target_symbol_in_states(program, states, transition.target)
        .and_then(|symbol| state_id_for_symbol(state_nodes, symbol))
}

fn transition_target_symbol_in_states(
    program: &CheckedTrees,
    states: &[State],
    target: TransitionTargetHandle,
) -> Option<SymbolHandle> {
    if !target.is_valid() {
        return None;
    }

    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named { path, .. } => states
            .iter()
            .find(|state| state.symbol == path.symbol)
            .map(|state| state.symbol),
        TransitionTargetNode::Value(_)
        | TransitionTargetNode::SelfTarget
        | TransitionTargetNode::Terminal => None,
    }
}

fn semantic_symbol_name(program: &CheckedTrees, symbol: SymbolHandle) -> String {
    for machine in program.machines() {
        if machine.symbol == symbol {
            return machine.name.as_str().to_owned();
        }
        for state in program.machine_states(machine) {
            if state.symbol == symbol {
                return state.name.as_str().to_owned();
            }
            for parameter in program.state_parameters(state) {
                if parameter.symbol == symbol {
                    return parameter.name.as_str().to_owned();
                }
            }
        }
        for owned in program.machine_owned_data(machine) {
            if owned.symbol == symbol {
                return owned.name.as_str().to_owned();
            }
        }
    }
    for data in program.data_definitions() {
        if data.symbol == symbol {
            return data.name.as_str().to_owned();
        }
        for member in program.data_members(data) {
            match member {
                omega_typed_trees::data::DataMember::Field(field) if field.symbol == symbol => {
                    return field.name.as_str().to_owned();
                }
                omega_typed_trees::data::DataMember::Variant(variant)
                    if variant.symbol == symbol =>
                {
                    return variant.name.as_str().to_owned();
                }
                _ => {}
            }
        }
    }
    if let Some(domain) = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == symbol)
    {
        return domain.name.to_string();
    }
    if let Some(invariant) = program
        .invariant_definitions()
        .iter()
        .find(|invariant| invariant.symbol == symbol)
    {
        return invariant.name.to_string();
    }
    if let Some(trait_definition) = program
        .traits()
        .iter()
        .find(|trait_definition| trait_definition.symbol == symbol)
    {
        return trait_definition.name.as_str().to_owned();
    }
    program.symbols.name(symbol).to_string()
}

fn state_label_from_symbol(program: &CheckedTrees, symbol: SymbolHandle) -> String {
    program
        .machines()
        .iter()
        .find_map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == symbol)
                .map(|state| format!("{}::{}", machine.name.as_str(), state.name.as_str()))
        })
        .unwrap_or_else(|| symbol_label(program, symbol))
}

fn symbol_label(program: &CheckedTrees, symbol: SymbolHandle) -> String {
    if symbol.is_valid() {
        format!(
            "{} (#{})",
            program.symbols.name(symbol),
            symbol.arena_index()
        )
    } else {
        "invalid".to_owned()
    }
}

fn push_json_string(output: &mut String, value: &str) {
    output.push('"');
    for ch in value.chars() {
        match ch {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            c if c.is_control() => {
                use std::fmt::Write;
                let _ = write!(output, "\\u{:04x}", c as u32);
            }
            c => output.push(c),
        }
    }
    output.push('"');
}

#[cfg(test)]
mod tests {
    use super::{
        carry_manifest_json, claim_outcome_manifest_json, machine_contract_manifest_json,
        push_termination_interface_json, qualification_evidence_manifest_json,
    };
    use omega_checked_trees::{
        CheckedTrees, ClaimCarryPolicyFact, DataCarryFact, FlowClaimOutcomeEntryFact,
        FlowClaimOutcomeMapFact, FlowClaimOutcomeSource, MachineActivationCarryFact,
        MachineContractPlan, MachineTerminationFact,
    };
    use omega_core::semantics::{
        BlockingInterface, BlockingPlan, CarryAddress, CarryCpu, CarryHostThread, CarryPolicy,
        CarrySuspension, MachineSupplyMode, MachineTerminationPlan, QualificationEvidenceOrigin,
        RankingViewId, RankingWitness, SuspensionInterface, SuspensionPlan, TerminationGuarantee,
        TerminationInterface,
    };
    use omega_core::symbols::SymbolHandle;
    use omega_facts::{
        Fact, FactOrigin, FactPayload, FactPlace, ProgramPoint, QualificationEvidence,
    };
    use omega_typed_trees::machine::Machine;
    use omega_typed_trees::name::Identifier;
    use omega_typed_trees::typed_trees::MachineSpecialization;

    #[test]
    fn claim_outcome_manifest_keeps_paths_and_source_kinds_structured() {
        let mut program = CheckedTrees::default();
        let output_segments = program.facts.flow.ownership.segments.insert_many([
            omega_facts::PlaceSegment::Case {
                variant: SymbolHandle::invalid(),
            },
            omega_facts::PlaceSegment::Field {
                symbol: SymbolHandle::invalid(),
            },
        ]);
        let entries = program
            .facts
            .flow
            .ownership
            .claim_outcome_entries
            .insert_many([
                FlowClaimOutcomeEntryFact {
                    output_segments,
                    source: FlowClaimOutcomeSource::Input {
                        parameter_symbol: SymbolHandle::invalid(),
                        segments: Default::default(),
                    },
                },
                FlowClaimOutcomeEntryFact {
                    output_segments: Default::default(),
                    source: FlowClaimOutcomeSource::Established {
                        claim_identity:
                            omega_core::semantics::PermissionClaimIdentity::Established {
                                machine_symbol: SymbolHandle::invalid(),
                                state_symbol: SymbolHandle::invalid(),
                                source: omega_core::semantics::PermissionEventSource::Statement {
                                    statement_index: 2,
                                },
                                ordinal: 7,
                            },
                        provenance: omega_core::semantics::PermissionProvenance::Established {
                            machine_symbol: SymbolHandle::invalid(),
                            state_symbol: SymbolHandle::invalid(),
                            source: omega_core::semantics::PermissionEventSource::StateEntry,
                        },
                    },
                },
            ]);
        program
            .facts
            .flow
            .ownership
            .claim_outcome_maps
            .insert(FlowClaimOutcomeMapFact {
                machine_symbol: SymbolHandle::invalid(),
                state_symbol: SymbolHandle::invalid(),
                entries,
            });

        let json = claim_outcome_manifest_json(&program);

        assert!(json.contains("\"claim_outcome_maps\""));
        assert!(
            json.contains("\"output_path\": [{\"case\": \"invalid\"}, {\"field\": \"invalid\"}]")
        );
        assert!(json.contains("\"kind\": \"input\""));
        assert!(json.contains("\"kind\": \"established\""));
        assert!(json.contains("\"statement_index\": 2"));
        assert!(json.contains("\"ordinal\": 7"));
        assert!(json.contains("\"kind\": \"state_entry\""));
    }

    #[test]
    fn qualification_evidence_manifest_separates_origin_point_and_receipt() {
        let subject = SymbolHandle::from_arena_index(4);
        let domain = SymbolHandle::from_arena_index(5);
        let provider = SymbolHandle::from_arena_index(6);
        let mut program = CheckedTrees::default();
        let place = program.facts.semantic.append_symbol_place(subject);
        program.facts.semantic.append_fact(Fact {
            place: FactPlace::Place(place),
            point: ProgramPoint::CallEnsures {
                machine_symbol: subject,
                state_symbol: subject,
                statement_index: 2,
                call_ordinal: 1,
            },
            origin: FactOrigin::CallEnsures,
            evidence: QualificationEvidence {
                origin: QualificationEvidenceOrigin::AdmittedReceipt,
                source_symbol: provider,
                requirement_symbol: SymbolHandle::invalid(),
                receipt_identity: 0x1234,
            },
            payload: FactPayload::DomainMembership {
                value: Default::default(),
                domain: Default::default(),
                domain_symbol: domain,
            },
        });

        let json = qualification_evidence_manifest_json(&program);

        assert!(json.contains("\"subject\": \"#4\""));
        assert!(json.contains("\"domain\": \"#5\""));
        assert!(json.contains("\"origin\": \"admitted_receipt\""));
        assert!(json.contains("\"program_point\": \"call_ensures\""));
        assert!(json.contains("\"source\": \"#6\""));
        assert!(json.contains("\"requirement\": null"));
        assert!(json.contains("\"receipt_identity\": \"0x0000000000001234\""));
    }

    #[test]
    fn carry_manifest_keeps_authored_and_effective_policies_separate() {
        let symbol = SymbolHandle::from_arena_index(7);
        let declared = CarryPolicy {
            suspension: CarrySuspension::Forbidden,
            cpu: CarryCpu::Origin,
            host_thread: CarryHostThread::Any,
            address: CarryAddress::Stable,
        };
        let mut program = CheckedTrees::default();
        program
            .typed
            .push_data_definition(omega_typed_trees::data::DataDefinition {
                symbol,
                name: Identifier::generated("PerCpuLease"),
                ..Default::default()
            });
        program.facts.carry.data.push(DataCarryFact {
            data: symbol,
            declared: Some(declared),
            effective: CarryPolicy::PERMISSIVE,
        });
        let machine = SymbolHandle::from_arena_index(8);
        program.typed.push_machine(Machine {
            symbol: machine,
            name: Identifier::generated("Worker::run"),
            ..Default::default()
        });
        program
            .facts
            .carry
            .activation_wide_carry
            .push(MachineActivationCarryFact {
                machine,
                effective: CarryPolicy::STRICT,
                analysis_complete: true,
                contributing_types: Vec::new(),
                unnamed_strict_values: 1,
            });
        program
            .facts
            .carry
            .claim_policies
            .push(ClaimCarryPolicyFact {
                claim_identity: omega_core::semantics::PermissionClaimIdentity::Unknown,
                effective: CarryPolicy::STRICT,
                contributing_origins: 2,
            });

        let json = carry_manifest_json(&program);

        assert!(json.contains("\"type\": \"PerCpuLease\""));
        assert!(json.contains(
            "\"declared\": {\"suspension\": \"forbidden\", \"cpu\": \"same\", \"thread\": \"any\", \"address\": \"stable\"}"
        ));
        assert!(json.contains(
            "\"effective\": {\"suspension\": \"allowed\", \"cpu\": \"any\", \"thread\": \"any\", \"address\": \"movable\"}"
        ));
        assert!(json.contains("\"machine\": \"Worker::run\""));
        assert!(json.contains("\"analysis_complete\": true"));
        assert!(json.contains("\"subtree_machine_count\": 1"));
        assert!(json.contains("\"unnamed_strict_values\": 1"));
        assert!(json.contains("\"claim_policies\": ["));
        assert!(json.contains("\"claim_identity\": {\"kind\": \"unknown\"}"));
        assert!(json.contains("\"contributing_origins\": 2"));
    }

    #[test]
    fn machine_contract_manifest_keeps_interface_and_witness_separate() {
        let symbol = SymbolHandle::from_arena_index(2);
        let service_symbol = SymbolHandle::from_arena_index(1);
        let mut program = CheckedTrees::default();
        let service = program
            .facts
            .service_reaches
            .services
            .intern(service_symbol, "Readable");
        let service_row = program.facts.service_reaches.rows.intern(vec![service]);
        program.typed.push_machine(Machine {
            symbol,
            name: Identifier::generated("Worker::run"),
            termination_plan: MachineTerminationPlan {
                implementation_witness: Some(RankingWitness {
                    subjects: vec!["remaining".to_string()],
                    ranking_view: RankingViewId::NAT_DESCENDING,
                    view_path: "Nat::Descending".to_string(),
                    view_arguments: Vec::new(),
                    rank_range: None,
                }),
                ..Default::default()
            },
            ..Default::default()
        });
        program
            .facts
            .contract_plans
            .machines
            .push(MachineContractPlan {
                machine: symbol,
                supply_mode: MachineSupplyMode::CheckedBody,
                service_reach: omega_core::semantics::ServiceReachPlan {
                    interface: omega_core::semantics::ServiceReachInterface::PublishedCeiling(
                        service_row,
                    ),
                    checked_inferred: service_row,
                },
                suspension: SuspensionPlan {
                    interface: SuspensionInterface::PublishedMaySuspend(false),
                    checked_may_suspend: false,
                },
                blocking: BlockingPlan {
                    interface: BlockingInterface::PublishedMayBlock(true),
                    checked_may_block: true,
                },
                termination: omega_core::semantics::TerminationInterface::Published(
                    TerminationGuarantee::NoGuarantee,
                ),
                inferred_write_frames: Vec::new(),
                fingerprint: 0x1234,
            });
        program
            .facts
            .termination
            .machines
            .push(MachineTerminationFact {
                machine: symbol,
                checked_summary: TerminationGuarantee::EventualTerminal {
                    premises: Vec::new(),
                },
                resolved_view_path: "Nat::Descending".to_string(),
            });

        let json = machine_contract_manifest_json(&program);
        let contract_start = json.find("\"contract\"").expect("contract object");
        let implementation_start = json
            .find("\"implementation\"")
            .expect("implementation object");
        let contract = &json[contract_start..implementation_start];

        assert!(contract.contains("\"fingerprint\": \"0x0000000000001234\""));
        assert!(contract.contains(
            "\"service_reach\": {\"interface\": \"published_ceiling\", \"services\": [\"Readable\"]}"
        ));
        assert!(contract.contains(
            "\"suspension\": {\"interface\": \"published_ceiling\", \"may_suspend\": false}"
        ));
        assert!(
            contract.contains(
                "\"blocking\": {\"interface\": \"published_ceiling\", \"may_block\": true}"
            )
        );
        assert!(contract.contains(
            "\"termination\": {\"interface\": \"published\", \"guarantee\": {\"kind\": \"no_guarantee\"}}"
        ));
        assert!(!contract.contains("inferred_write_frames"));
        assert!(!contract.contains("remaining"));
        assert!(json[implementation_start..].contains("\"inferred_write_frames\": []"));
        assert!(json[implementation_start..].contains("\"checked_may_suspend\": false"));
        assert!(json[implementation_start..].contains("\"checked_may_block\": true"));
        assert!(json[implementation_start..].contains("\"checked_service_reach\": [\"Readable\"]"));
        assert!(json[implementation_start..].contains("\"kind\": \"eventual_terminal\""));
        assert!(json[implementation_start..].contains("\"subjects\": [\"remaining\"]"));
        assert!(json[implementation_start..].contains("\"view\": \"Nat::Descending\""));
    }

    #[test]
    fn termination_manifest_distinguishes_private_derivation_from_public_omission() {
        let mut internal = String::new();
        push_termination_interface_json(&mut internal, &TerminationInterface::InternalDerived);
        assert_eq!(internal, "{\"interface\": \"internal_derived\"}");

        let mut omitted = String::new();
        push_termination_interface_json(
            &mut omitted,
            &TerminationInterface::Published(TerminationGuarantee::NoGuarantee),
        );
        assert_eq!(
            omitted,
            "{\"interface\": \"published\", \"guarantee\": {\"kind\": \"no_guarantee\"}}"
        );
    }

    #[test]
    fn machine_contract_manifest_records_specialization_trust_and_contract_ids() {
        let symbol = SymbolHandle::from_arena_index(3);
        let mut program = CheckedTrees::default();
        program.typed.push_machine(Machine {
            symbol,
            name: Identifier::generated("accepted_map"),
            supply_mode: MachineSupplyMode::Accepted,
            ..Default::default()
        });
        program
            .typed
            .machine_specializations
            .push(MachineSpecialization {
                template: symbol,
                instance: symbol,
                type_arguments: vec!["Card".to_owned()],
                machine_arguments: vec![SymbolHandle::from_arena_index(8)],
                template_contract_fingerprint: 0x1111,
                accepted_template_commitment: Some("accepted_map".to_owned()),
                machine_argument_contract_fingerprints: vec![0x2222],
                fingerprint: 0x3333,
            });

        let json = machine_contract_manifest_json(&program);
        assert!(json.contains("\"template\": \"accepted_map\""));
        assert!(json.contains("\"accepted_template_commitment\": \"accepted_map\""));
        assert!(json.contains("\"template_contract_fingerprint\": \"0x0000000000001111\""));
        assert!(
            json.contains("\"machine_argument_contract_fingerprints\": [\"0x0000000000002222\"]")
        );
        assert!(json.contains("\"instance_fingerprint\": \"0x0000000000003333\""));
    }
}
