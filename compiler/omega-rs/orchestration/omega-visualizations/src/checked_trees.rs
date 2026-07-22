use crate::phase_diagram::PhaseDiagramBuilder;
use omega_checked_trees::{
    BorrowAccessKind, BorrowArgumentAccessFact, BorrowLoanFact, CheckedTrees,
    FlowBorrowActivationFact, FlowBorrowWeakeningFact, FlowBorrowWeakeningReason, FlowCallFact,
    FlowInvalidationSource, FlowStateFact,
};
use omega_core::symbols::SymbolHandle;
use omega_effects::{CapabilityFlowKind, EffectSet};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::state::State;
use omega_typed_trees::statement::{
    StatementNode, TableTransition, TransitionTargetHandle, TransitionTargetNode,
};

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
        if let Some(effects) = machine_effects_for(program, machine.symbol) {
            diagram.node_effects(&machine_id, effect_names_from_set(effects.transitive));
        }
        machine_nodes.push((machine.symbol, machine_id.clone()));

        for state in program.machine_states(machine) {
            let state_id = diagram.node(
                format!("state_{machine_index}_{}", state.symbol.arena_index()),
                state_label(program, machine, state),
                "state_block",
                machine_index + 1,
            );
            if let Some(flow_state) = flow_state_for(program, machine.symbol, state.symbol) {
                diagram.node_effects(
                    &state_id,
                    effect_names_from_set(flow_state.transitive_effects),
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

pub fn capability_manifest_html(program: &CheckedTrees) -> String {
    crate::phase_diagram::text_report_html(
        "capability_manifest",
        &capability_manifest_text(program),
    )
}

pub fn capability_manifest_json(program: &CheckedTrees) -> String {
    let manifest = entry_capability_manifest(program);
    let effect_names = manifest.effects.names().collect::<Vec<_>>();

    let mut json = String::new();
    json.push_str("{\n");
    json.push_str("  \"entry_machine\": ");
    push_json_string(&mut json, &manifest.entry_machine);
    json.push_str(",\n  \"entry_state\": ");
    push_json_string(&mut json, &manifest.entry_state);
    json.push_str(",\n  \"effect_bits\": \"0x");
    json.push_str(&format!("{:016x}", manifest.effects.bits()));
    json.push_str("\",\n  \"effects\": [");
    for (index, effect) in effect_names.iter().enumerate() {
        if index > 0 {
            json.push_str(", ");
        }
        push_json_string(&mut json, effect);
    }
    json.push_str("],\n  \"capability_flows\": {");
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
    json.push_str("\n  ],\n  \"asynchronous_preemption\": [");
    for (index, fact) in program
        .facts
        .carry
        .asynchronous_preemption
        .iter()
        .enumerate()
    {
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

/// Provider-independent task activation demands. Runtime/provider admission
/// consumes these normalized facts; the artifact keeps target/layout and
/// canonical carry derivation inspectable without exposing provider handles.
pub fn task_activation_manifest_json(program: &CheckedTrees) -> String {
    use omega_checked_trees::{TaskStartOperation, machine::Machine};
    use omega_task_plans::{
        AddressStabilityDemand, DistinctActivationRequirement, SameCpuDemand, SameThreadDemand,
    };

    fn machine_name<'a>(machines: &'a [Machine], symbol: SymbolHandle) -> &'a str {
        machines
            .iter()
            .find(|machine| machine.symbol == symbol)
            .map(|machine| machine.name.as_str())
            .unwrap_or("<unknown>")
    }
    fn push_migration(json: &mut String, demand: omega_task_plans::MigrationDemand) {
        json.push_str("{\"cpu\": ");
        push_json_string(
            json,
            match demand.cpu {
                SameCpuDemand::Any => "any",
                SameCpuDemand::Same => "same",
            },
        );
        json.push_str(", \"thread\": ");
        push_json_string(
            json,
            match demand.thread {
                SameThreadDemand::Any => "any",
                SameThreadDemand::Same => "same",
            },
        );
        json.push_str(", \"address\": ");
        push_json_string(
            json,
            match demand.address {
                AddressStabilityDemand::Movable => "movable",
                AddressStabilityDemand::Stable => "stable",
            },
        );
        json.push('}');
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
        json.push_str("\",\n      \"continuation\": {\"bytes\": ");
        json.push_str(&plan.continuation_bytes.to_string());
        json.push_str(", \"alignment\": ");
        json.push_str(&plan.continuation_alignment.to_string());
        json.push_str("},\n      \"reaches_suspend\": ");
        json.push_str(if plan.reaches_suspend {
            "true"
        } else {
            "false"
        });
        json.push_str(",\n      \"suspension_crossings_safe\": ");
        json.push_str(if plan.suspension_crossings_safe {
            "true"
        } else {
            "false"
        });
        json.push_str(",\n      \"safe_point_migration\": ");
        push_migration(&mut json, plan.safe_point_migration);
        json.push_str(",\n      \"asynchronous_migration\": ");
        if let Some(demand) = plan.asynchronous_migration {
            push_migration(&mut json, demand);
        } else {
            json.push_str("null");
        }
        json.push_str(",\n      \"cancellation_required\": ");
        json.push_str(if plan.cancellation_required {
            "true"
        } else {
            "false"
        });
        json.push_str(",\n      \"activation\": ");
        push_json_string(
            &mut json,
            match plan.activation {
                DistinctActivationRequirement::Required => "distinct_required",
                DistinctActivationRequirement::InlineCompletionAllowed => {
                    "inline_completion_allowed"
                }
            },
        );
        // The checked plan is an admitted demand only after an identified
        // runtime provider supplies a behavior contract and the normalized
        // join succeeds. Until provider identity/provenance is present, keep
        // that absence visible instead of rendering a permissive default.
        json.push_str(",\n      \"runtime_admission\": {\"status\": \"pending_provider\"}");
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
            json.push_str(",\n        \"published_effect_row\": ");
            json.push_str(&contract.published_effect_row.0.to_string());
            json.push_str(",\n        \"published_termination\": ");
            push_termination_json(&mut json, &contract.published_termination);
            json.push_str("\n      }");
        } else {
            json.push_str("}");
        }

        json.push_str(",\n      \"implementation\": {");
        let mut has_implementation_field = false;
        if let Some(contract) = program.facts.contract_plans.for_machine(machine.symbol) {
            json.push_str("\n        \"inferred_write_frames\": [");
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
    if let Some(effects) = machine_effects_for(program, machine.symbol) {
        append_effect_lines(&mut label, effects.direct, effects.transitive);
    }
    label
}

fn state_label(program: &CheckedTrees, machine: &Machine, state: &State) -> String {
    let borrow_state = borrow_state_for(program, machine.symbol, state.symbol);
    let flow_state = flow_state_for(program, machine.symbol, state.symbol);

    let writable_root_count = borrow_state
        .map(|borrow| borrow.writable_roots.len())
        .unwrap_or(0);
    let (invalidation_count, mutable_parameter_count, direct_effects, reached_effects) =
        if let Some(flow) = flow_state {
            (
                flow.invalidations.len(),
                flow.mutable_parameter_count,
                flow.direct_effects,
                flow.transitive_effects,
            )
        } else {
            (
                0,
                borrow_state
                    .map(|borrow| borrow.mutable_parameter_count)
                    .unwrap_or(0),
                EffectSet::empty(),
                EffectSet::empty(),
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
    append_effect_lines(&mut label, direct_effects, reached_effects);

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

        diagram.node_effects(&rendered_id, effect_names_from_set(call.transitive_effects));
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
    append_effect_lines(&mut label, call.direct_effects, call.transitive_effects);
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

    if let Some(contained) = program
        .machine_contained_objects(machine)
        .iter()
        .find(|contained| contained.symbol == symbol)
    {
        return contained.name.as_str().to_owned();
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

fn machine_effects_for(
    program: &CheckedTrees,
    symbol: SymbolHandle,
) -> Option<&omega_effects::MachineEffects> {
    program
        .facts
        .effects
        .machines()
        .iter()
        .find(|effects| effects.symbol == symbol)
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

fn format_effect_set(effects: EffectSet) -> String {
    if effects.is_empty() {
        return "<none> [0x0000000000000000]".to_owned();
    }

    format!(
        "{} [0x{:016x}]",
        effects.names().collect::<Vec<_>>().join(", "),
        effects.bits()
    )
}

fn append_effect_lines(label: &mut String, direct: EffectSet, reached: EffectSet) {
    if !direct.is_empty() {
        label.push_str("\ndirect effects: ");
        label.push_str(&format_effect_set(direct));
    }
    if !reached.is_empty() {
        label.push_str("\nreached effects: ");
        label.push_str(&format_effect_set(reached));
    }
}

fn effect_names_from_set(effects: EffectSet) -> Vec<String> {
    effects.names().map(str::to_owned).collect()
}

fn capability_manifest_text(program: &CheckedTrees) -> String {
    let manifest = entry_capability_manifest(program);
    let mut report = String::new();

    report.push_str("Executable Capability Manifest\n");
    report.push_str("==============================\n\n");
    report.push_str("entry machine: ");
    report.push_str(&manifest.entry_machine);
    report.push('\n');
    report.push_str("entry state:   ");
    report.push_str(&manifest.entry_state);
    report.push('\n');
    report.push_str("effects:       ");
    report.push_str(&format_effect_set(manifest.effects));
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
    effects: EffectSet,
    capability_flow_counts: [(CapabilityFlowKind, usize); 5],
}

fn entry_capability_manifest(program: &CheckedTrees) -> EntryCapabilityManifest {
    let Some((machine_symbol, machine_name, state_name)) = entry_machine(program) else {
        return EntryCapabilityManifest {
            entry_machine: "<missing>".to_owned(),
            entry_state: "<missing>".to_owned(),
            effects: EffectSet::empty(),
            capability_flow_counts: capability_flow_counts(program),
        };
    };

    let effects = program
        .facts
        .effects
        .machines()
        .iter()
        .find(|effects| effects.symbol == machine_symbol)
        .map(|effects| effects.transitive)
        .unwrap_or_else(EffectSet::empty);

    EntryCapabilityManifest {
        entry_machine: machine_name,
        entry_state: state_name,
        effects,
        capability_flow_counts: capability_flow_counts(program),
    }
}

fn capability_flow_counts(program: &CheckedTrees) -> [(CapabilityFlowKind, usize); 5] {
    CapabilityFlowKind::ALL.map(|kind| (kind, program.facts.capabilities.count_by_kind(kind)))
}

fn entry_machine(program: &CheckedTrees) -> Option<(SymbolHandle, String, String)> {
    entry_machine_with_state(program, "Main::main", "main")
        .or_else(|| entry_machine_with_state(program, "main", "entry"))
}

fn entry_machine_with_state(
    program: &CheckedTrees,
    machine_name: &str,
    state_name: &str,
) -> Option<(SymbolHandle, String, String)> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)?;
    program
        .machine_states(machine)
        .iter()
        .any(|state| state.name.as_str() == state_name)
        .then(|| {
            (
                machine.symbol,
                machine.name.as_str().to_owned(),
                state_name.to_owned(),
            )
        })
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
        for contained in program.machine_contained_objects(machine) {
            if contained.symbol == symbol {
                return contained.name.as_str().to_owned();
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
    use super::{carry_manifest_json, machine_contract_manifest_json};
    use omega_checked_trees::{
        CheckedTrees, DataCarryFact, MachineContractPlan, MachinePreemptionCarryFact,
        MachineTerminationFact,
    };
    use omega_core::semantics::{
        CarryAddress, CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension, EffectRowId,
        MachineSupplyMode, MachineTerminationPlan, RankingViewId, RankingWitness,
        TerminationGuarantee,
    };
    use omega_core::symbols::SymbolHandle;
    use omega_typed_trees::machine::Machine;
    use omega_typed_trees::name::Identifier;
    use omega_typed_trees::typed_trees::MachineSpecialization;

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
            .asynchronous_preemption
            .push(MachinePreemptionCarryFact {
                machine,
                effective: CarryPolicy::STRICT,
                analysis_complete: true,
                contributing_types: Vec::new(),
                unnamed_strict_values: 1,
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
        assert!(json.contains("\"unnamed_strict_values\": 1"));
    }

    #[test]
    fn machine_contract_manifest_keeps_interface_and_witness_separate() {
        let symbol = SymbolHandle::default();
        let mut program = CheckedTrees::default();
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
                published_effect_row: EffectRowId::NULL,
                published_termination: TerminationGuarantee::NoGuarantee,
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
        assert!(contract.contains("\"kind\": \"no_guarantee\""));
        assert!(!contract.contains("inferred_write_frames"));
        assert!(!contract.contains("remaining"));
        assert!(json[implementation_start..].contains("\"inferred_write_frames\": []"));
        assert!(json[implementation_start..].contains("\"kind\": \"eventual_terminal\""));
        assert!(json[implementation_start..].contains("\"subjects\": [\"remaining\"]"));
        assert!(json[implementation_start..].contains("\"view\": \"Nat::Descending\""));
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
