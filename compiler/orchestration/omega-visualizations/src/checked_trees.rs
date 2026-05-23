use omega_checked_trees::Program;
use omega_core::symbols::SymbolHandle;
use omega_effects::EffectSet;

pub fn checked_trees_html(program: &Program) -> String {
    crate::phase_diagram::text_report_html("checked_trees", &checked_effects_report(program))
}

pub fn capability_manifest_html(program: &Program) -> String {
    crate::phase_diagram::text_report_html(
        "capability_manifest",
        &capability_manifest_text(program),
    )
}

pub fn capability_manifest_json(program: &Program) -> String {
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
    json.push_str("]\n}\n");
    json
}

fn checked_effects_report(program: &Program) -> String {
    let mut report = String::new();
    report.push_str("Checked Facts\n");
    report.push_str("=============\n\n");
    report.push_str("Effects\n");
    report.push_str("-------\n");
    report.push_str("Effects are stored as propagated bitsets on checked-tree facts.\n");
    report.push_str("direct = declared or boundary effects at that node.\n");
    report.push_str("reached = direct effects plus effects reached through calls.\n\n");

    for machine_effects in program.facts.effects.machines() {
        let machine_name = machine_name(program, machine_effects.symbol);
        report.push_str("machine ");
        report.push_str(&machine_name);
        report.push('\n');
        report.push_str("  symbol: ");
        report.push_str(&symbol_label(machine_effects.symbol));
        report.push('\n');
        report.push_str("  direct:  ");
        report.push_str(&format_effect_set(machine_effects.direct));
        report.push('\n');
        report.push_str("  reached: ");
        report.push_str(&format_effect_set(machine_effects.transitive));
        report.push('\n');

        for state_effects in program
            .facts
            .effects
            .states
            .span_or_empty(machine_effects.states)
        {
            let current_state_name = state_name(program, state_effects.symbol);
            report.push_str("  state ");
            report.push_str(&current_state_name);
            report.push('\n');
            report.push_str("    symbol: ");
            report.push_str(&symbol_label(state_effects.symbol));
            report.push('\n');
            report.push_str("    direct:  ");
            report.push_str(&format_effect_set(state_effects.direct));
            report.push('\n');
            report.push_str("    reached: ");
            report.push_str(&format_effect_set(state_effects.transitive));
            report.push('\n');

            for call_effects in program
                .facts
                .effects
                .calls
                .span_or_empty(state_effects.calls)
            {
                report.push_str("    call ");
                report.push_str(&call_effects.statement_index.to_string());
                report.push('.');
                report.push_str(&call_effects.call_ordinal.to_string());
                report.push_str(" -> ");
                report.push_str(&state_name(program, call_effects.target_state_symbol));
                report.push('\n');
                report.push_str("      direct:  ");
                report.push_str(&format_effect_set(call_effects.direct));
                report.push('\n');
                report.push_str("      reached: ");
                report.push_str(&format_effect_set(call_effects.transitive));
                report.push('\n');
            }
        }

        report.push('\n');
    }

    report.push_str("Semantic Fact Spine\n");
    report.push_str("-------------------\n");
    report.push_str(
        "Centralized fact storage: dense facts, dense refs, and context/symbol indexes.\n\n",
    );
    report.push_str("facts:       ");
    report.push_str(&program.facts.semantic.facts.len().to_string());
    report.push('\n');
    report.push_str("refs:        ");
    report.push_str(&program.facts.semantic.refs.len().to_string());
    report.push('\n');
    report.push_str("contexts:    ");
    report.push_str(&program.facts.semantic.contexts.len().to_string());
    report.push('\n');
    report.push_str("symbol sets: ");
    report.push_str(&program.facts.semantic.symbol_sets.len().to_string());
    report.push_str("\n\n");

    for (_, context) in program.facts.semantic.contexts.iter() {
        report.push_str("context ");
        report.push_str(&semantic_point_label(program, context.point));
        report.push('\n');
        for fact_ref in program.facts.semantic.refs.span_or_empty(context.facts) {
            let fact = program.facts.semantic.facts.get(fact_ref.fact);
            report.push_str("  ");
            report.push_str(&semantic_fact_label(program, fact));
            report.push('\n');
        }
    }

    if !program.facts.semantic.contexts.is_empty() {
        report.push('\n');
    }

    report.push_str("Flow Environments\n");
    report.push_str("-----------------\n");
    report.push_str(
        "Shared state/call/exit snapshots tie borrow roots, semantic contexts, contracts, and effects together.\n\n",
    );
    report.push_str("states:   ");
    report.push_str(&program.facts.flow.states.len().to_string());
    report.push('\n');
    report.push_str("calls:    ");
    report.push_str(&program.facts.flow.calls.len().to_string());
    report.push('\n');
    report.push_str("exits:    ");
    report.push_str(&program.facts.flow.exits.len().to_string());
    report.push('\n');
    report.push_str("contexts: ");
    report.push_str(&program.facts.flow.semantic_context_refs.len().to_string());
    report.push_str("\n\n");

    for (_, state_flow) in program.facts.flow.states.iter() {
        report.push_str("state ");
        report.push_str(&machine_name(program, state_flow.machine_symbol));
        report.push_str("::");
        report.push_str(&state_name(program, state_flow.state_symbol));
        report.push('\n');
        report.push_str("  writable roots: ");
        report.push_str(
            &program
                .facts
                .borrow
                .writable_roots
                .span_or_empty(state_flow.writable_roots)
                .len()
                .to_string(),
        );
        report.push('\n');
        report.push_str("  mutable params: ");
        report.push_str(&state_flow.mutable_parameter_count.to_string());
        report.push('\n');
        report.push_str("  contexts: ");
        append_flow_context_labels(program, &mut report, state_flow.semantic_contexts);
        report.push('\n');
        report.push_str("  direct effects:  ");
        report.push_str(&format_effect_set(state_flow.direct_effects));
        report.push('\n');
        report.push_str("  reached effects: ");
        report.push_str(&format_effect_set(state_flow.transitive_effects));
        report.push('\n');

        for call_flow in program.facts.flow.calls.span_or_empty(state_flow.calls) {
            report.push_str("  call ");
            report.push_str(&call_flow.statement_index.to_string());
            report.push('.');
            report.push_str(&call_flow.call_ordinal.to_string());
            report.push_str(" -> ");
            report.push_str(&state_label_from_symbol(program, call_flow.target_symbol));
            report.push('\n');
            report.push_str("    contexts: ");
            append_flow_context_labels(program, &mut report, call_flow.semantic_contexts);
            report.push('\n');
            report.push_str("    requires: ");
            append_contract_fact_ref_summary(program, &mut report, call_flow.requires);
            report.push('\n');
            report.push_str("    ensures: ");
            append_contract_fact_ref_summary(program, &mut report, call_flow.ensures);
            report.push('\n');
            report.push_str("    direct effects:  ");
            report.push_str(&format_effect_set(call_flow.direct_effects));
            report.push('\n');
            report.push_str("    reached effects: ");
            report.push_str(&format_effect_set(call_flow.transitive_effects));
            report.push('\n');
        }

        for exit_flow in program.facts.flow.exits.span_or_empty(state_flow.exits) {
            report.push_str("  exit ");
            report.push_str(&exit_flow.statement_index.to_string());
            report.push('\n');
            report.push_str("    contexts: ");
            append_flow_context_labels(program, &mut report, exit_flow.semantic_contexts);
            report.push('\n');
            report.push_str("    ensures: ");
            append_contract_fact_ref_summary(program, &mut report, exit_flow.ensures);
            report.push('\n');
        }

        report.push('\n');
    }

    report.push_str("Contract Facts\n");
    report.push_str("--------------\n");
    report.push_str("Contracts are stored as checked-tree proof facts pointing into typed proof fact arenas.\n\n");

    for (_, fact) in program.facts.proof.contract_facts.iter() {
        report.push_str("contract fact ");
        report.push_str(&contract_fact_kind(fact));
        report.push('\n');
        report.push_str("  owner: ");
        report.push_str(&contract_fact_owner(program, fact));
        report.push('\n');
        report.push_str("  fact:  ");
        report.push_str(&typed_proof_fact_label(program, fact.fact));
        report.push('\n');
    }

    if !program.facts.proof.contract_calls.is_empty() {
        report.push('\n');
    }
    for (_, call) in program.facts.proof.contract_calls.iter() {
        report.push_str("call ");
        report.push_str(&machine_name(program, call.caller_machine_symbol));
        report.push_str("::");
        report.push_str(&state_name(program, call.caller_state_symbol));
        report.push(' ');
        report.push_str(&call.statement_index.to_string());
        report.push('.');
        report.push_str(&call.call_ordinal.to_string());
        report.push_str(" -> ");
        report.push_str(&machine_name(program, call.target_machine_symbol));
        report.push_str("::");
        report.push_str(&state_name(program, call.target_state_symbol));
        report.push('\n');
        append_contract_fact_ref_list(program, &mut report, "requires", call.requires);
        append_contract_fact_ref_list(program, &mut report, "ensures", call.ensures);
    }

    if !program.facts.proof.contract_exits.is_empty() {
        report.push('\n');
    }
    for (_, exit) in program.facts.proof.contract_exits.iter() {
        report.push_str("exit ");
        report.push_str(&machine_name(program, exit.machine_symbol));
        report.push_str("::");
        report.push_str(&state_name(program, exit.state_symbol));
        report.push(' ');
        report.push_str(&exit.statement_index.to_string());
        report.push('\n');
        append_contract_fact_ref_list(program, &mut report, "ensures", exit.ensures);
    }

    report
}

fn semantic_point_label(program: &Program, point: omega_facts::ProgramPoint) -> String {
    match point {
        omega_facts::ProgramPoint::Global => "global".to_owned(),
        omega_facts::ProgramPoint::Definition { symbol } => {
            format!("definition {}", semantic_symbol_name(program, symbol))
        }
        omega_facts::ProgramPoint::Machine { machine_symbol } => {
            format!("machine {}", machine_name(program, machine_symbol))
        }
        omega_facts::ProgramPoint::State {
            machine_symbol,
            state_symbol,
        } => format!(
            "state {}::{}",
            machine_name(program, machine_symbol),
            state_name(program, state_symbol)
        ),
        omega_facts::ProgramPoint::Statement {
            machine_symbol,
            state_symbol,
            statement_index,
        } => format!(
            "statement {}::{} {statement_index}",
            machine_name(program, machine_symbol),
            state_name(program, state_symbol)
        ),
        omega_facts::ProgramPoint::Call {
            machine_symbol,
            state_symbol,
            statement_index,
            call_ordinal,
        } => format!(
            "call {}::{} {statement_index}.{call_ordinal}",
            machine_name(program, machine_symbol),
            state_name(program, state_symbol)
        ),
        omega_facts::ProgramPoint::Exit {
            machine_symbol,
            state_symbol,
            statement_index,
        } => format!(
            "exit {}::{} {statement_index}",
            machine_name(program, machine_symbol),
            state_name(program, state_symbol)
        ),
    }
}

fn semantic_fact_label(program: &Program, fact: &omega_facts::Fact) -> String {
    let origin = semantic_origin_label(program, fact.origin);
    let payload = semantic_payload_label(program, fact.payload);
    let place = semantic_place_label(program, fact.place);
    format!("{payload} | place: {place} | origin: {origin}")
}

fn semantic_payload_label(program: &Program, payload: omega_facts::FactPayload) -> String {
    match payload {
        omega_facts::FactPayload::BooleanExpression(expression) => {
            program.expression_table.display_name(expression)
        }
        omega_facts::FactPayload::ContractBooleanExpression {
            kind,
            fact,
            expression,
        } => format!(
            "{} contract {} ({})",
            semantic_contract_kind(kind),
            program.expression_table.display_name(expression),
            typed_proof_fact_label(program, fact)
        ),
        omega_facts::FactPayload::DomainMembership {
            value,
            domain: _,
            domain_symbol,
        } => format!(
            "{} in {}",
            program.expression_table.display_name(value),
            semantic_symbol_name(program, domain_symbol)
        ),
        omega_facts::FactPayload::ContractDomainMembership {
            kind,
            fact,
            value,
            domain: _,
            domain_symbol,
        } => format!(
            "{} contract {} in {} ({})",
            semantic_contract_kind(kind),
            program.expression_table.display_name(value),
            semantic_symbol_name(program, domain_symbol),
            typed_proof_fact_label(program, fact)
        ),
        omega_facts::FactPayload::TypeConstraint { constraint } => program
            .type_reference_table
            .constraints(omega_core::arena::HandleSpan::from_parts(constraint, 1))
            .first()
            .map(|constraint| constraint.display_name(&program.expression_table))
            .unwrap_or_else(|| "<missing constraint>".to_owned()),
        omega_facts::FactPayload::ProofObligation { kind } => {
            format!("proof obligation {}", semantic_proof_obligation_kind(kind))
        }
        omega_facts::FactPayload::Contract { kind, fact } => {
            format!(
                "{} contract {}",
                semantic_contract_kind(kind),
                typed_proof_fact_label(program, fact)
            )
        }
        omega_facts::FactPayload::InvariantDefinition { constraint_count } => {
            format!("invariant definition ({constraint_count} constraints)")
        }
    }
}

fn semantic_place_label(program: &Program, place: omega_facts::FactPlace) -> String {
    match place {
        omega_facts::FactPlace::Unknown => "unknown".to_owned(),
        omega_facts::FactPlace::Place(place) => {
            let place = program.facts.semantic.places.get(place);
            semantic_canonical_place_label(program, place)
        }
        omega_facts::FactPlace::Symbol(symbol) => semantic_symbol_name(program, symbol),
        omega_facts::FactPlace::Expression(expression) => {
            program.expression_table.display_name(expression)
        }
        omega_facts::FactPlace::TypeReference(type_reference) => {
            program.display_type_reference(type_reference)
        }
    }
}

fn semantic_canonical_place_label(program: &Program, place: &omega_facts::Place) -> String {
    let mut label = match place.root {
        omega_facts::PlaceRoot::Unknown => "unknown".to_owned(),
        omega_facts::PlaceRoot::Symbol(symbol) => semantic_symbol_name(program, symbol),
        omega_facts::PlaceRoot::Expression(expression) => {
            program.expression_table.display_name(expression)
        }
        omega_facts::PlaceRoot::TypeReference(type_reference) => {
            program.display_type_reference(type_reference)
        }
    };

    for segment in program
        .facts
        .semantic
        .place_segments
        .span_or_empty(place.segments)
    {
        match segment {
            omega_facts::PlaceSegment::Field { symbol } => {
                label.push('.');
                label.push_str(&semantic_symbol_name(program, *symbol));
            }
            omega_facts::PlaceSegment::Index { expression } => {
                label.push('[');
                label.push_str(&program.expression_table.display_name(*expression));
                label.push(']');
            }
        }
    }

    label
}

fn semantic_origin_label(program: &Program, origin: omega_facts::FactOrigin) -> String {
    match origin {
        omega_facts::FactOrigin::Unknown => "unknown".to_owned(),
        omega_facts::FactOrigin::DomainDefinition { domain_symbol } => {
            format!("domain {}", semantic_symbol_name(program, domain_symbol))
        }
        omega_facts::FactOrigin::InvariantDefinition { invariant_symbol } => {
            format!(
                "invariant {}",
                semantic_symbol_name(program, invariant_symbol)
            )
        }
        omega_facts::FactOrigin::TypeReference => "type reference".to_owned(),
        omega_facts::FactOrigin::ProofObligation => "proof obligation".to_owned(),
        omega_facts::FactOrigin::MachineContract { machine_symbol } => {
            format!("machine contract {}", machine_name(program, machine_symbol))
        }
        omega_facts::FactOrigin::StateSignatureContract {
            owner_symbol,
            state_symbol,
        } => format!(
            "signature contract {}::{}",
            signature_owner_name(program, owner_symbol),
            state_signature_name(program, state_symbol)
        ),
        omega_facts::FactOrigin::CallRequires => "call requires".to_owned(),
        omega_facts::FactOrigin::CallEnsures => "call ensures".to_owned(),
        omega_facts::FactOrigin::ExitEnsures => "exit ensures".to_owned(),
    }
}

fn semantic_contract_kind(kind: omega_facts::ContractFactKind) -> &'static str {
    match kind {
        omega_facts::ContractFactKind::Requires => "requires",
        omega_facts::ContractFactKind::Ensures => "ensures",
        omega_facts::ContractFactKind::Trusted => "trusted",
    }
}

fn semantic_proof_obligation_kind(kind: omega_facts::ProofObligationKind) -> &'static str {
    match kind {
        omega_facts::ProofObligationKind::BoundedAssignment => "bounded assignment",
        omega_facts::ProofObligationKind::BoundedCallArgument => "bounded call argument",
        omega_facts::ProofObligationKind::BoundedInitializer => "bounded initializer",
        omega_facts::ProofObligationKind::BoundedStateReturn => "bounded state return",
        omega_facts::ProofObligationKind::BoundedValue => "bounded value",
        omega_facts::ProofObligationKind::BoundedTransitionArgument => {
            "bounded transition argument"
        }
        omega_facts::ProofObligationKind::GuardedTransition => "guarded transition",
    }
}

fn semantic_symbol_name(program: &Program, symbol: SymbolHandle) -> String {
    if let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol)
    {
        return machine.name.as_str().to_owned();
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
    if let Some(platform) = program
        .platforms()
        .iter()
        .find(|platform| platform.symbol == symbol)
    {
        return platform.name.as_str().to_owned();
    }
    symbol_label(symbol)
}

fn capability_manifest_text(program: &Program) -> String {
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

    report
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EntryCapabilityManifest {
    entry_machine: String,
    entry_state: String,
    effects: EffectSet,
}

fn entry_capability_manifest(program: &Program) -> EntryCapabilityManifest {
    let Some((machine_symbol, machine_name, state_name)) = entry_machine(program) else {
        return EntryCapabilityManifest {
            entry_machine: "<missing>".to_owned(),
            entry_state: "<missing>".to_owned(),
            effects: EffectSet::empty(),
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
    }
}

fn entry_machine(program: &Program) -> Option<(SymbolHandle, String, String)> {
    entry_machine_with_state(program, "Main::main", "main")
        .or_else(|| entry_machine_with_state(program, "main", "entry"))
}

fn entry_machine_with_state(
    program: &Program,
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

fn machine_name(program: &Program, symbol: SymbolHandle) -> String {
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol)
        .map(|machine| machine.name.as_str().to_owned())
        .unwrap_or_else(|| symbol_label(symbol))
}

fn state_name(program: &Program, symbol: SymbolHandle) -> String {
    program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine).iter())
        .find(|state| state.symbol == symbol)
        .map(|state| state.name.as_str().to_owned())
        .unwrap_or_else(|| symbol_label(symbol))
}

fn contract_fact_kind(fact: &omega_checked_trees::ContractProofFact) -> &'static str {
    match fact.kind {
        omega_checked_trees::ContractProofFactKind::Requires => "requires",
        omega_checked_trees::ContractProofFactKind::Ensures => "ensures",
        omega_checked_trees::ContractProofFactKind::Trusted => "trusted",
    }
}

fn contract_fact_owner(program: &Program, fact: &omega_checked_trees::ContractProofFact) -> String {
    match fact.owner {
        omega_checked_trees::ContractProofFactOwner::Unknown => "unknown".to_owned(),
        omega_checked_trees::ContractProofFactOwner::Machine { machine_symbol } => {
            format!("machine {}", machine_name(program, machine_symbol))
        }
        omega_checked_trees::ContractProofFactOwner::MachineState {
            machine_symbol,
            state_symbol,
        } => format!(
            "machine state {}::{}",
            machine_name(program, machine_symbol),
            state_name(program, state_symbol)
        ),
        omega_checked_trees::ContractProofFactOwner::StateSignature {
            owner_symbol,
            state_symbol,
        } => format!(
            "signature {}::{}",
            signature_owner_name(program, owner_symbol),
            state_signature_name(program, state_symbol)
        ),
    }
}

fn signature_owner_name(program: &Program, symbol: SymbolHandle) -> String {
    program
        .traits()
        .iter()
        .find(|trait_definition| trait_definition.symbol == symbol)
        .map(|trait_definition| trait_definition.name.as_str().to_owned())
        .or_else(|| {
            program
                .platforms()
                .iter()
                .find(|platform| platform.symbol == symbol)
                .map(|platform| platform.name.as_str().to_owned())
        })
        .unwrap_or_else(|| symbol_label(symbol))
}

fn state_signature_name(program: &Program, symbol: SymbolHandle) -> String {
    program
        .traits()
        .iter()
        .flat_map(|trait_definition| program.trait_machine_signatures(trait_definition))
        .find(|signature| signature.symbol == symbol)
        .map(|signature| signature.name.as_str().to_owned())
        .or_else(|| {
            program
                .platforms()
                .iter()
                .flat_map(|platform| program.platform_state_signatures(platform))
                .find(|signature| signature.symbol == symbol)
                .map(|signature| signature.name.as_str().to_owned())
        })
        .unwrap_or_else(|| symbol_label(symbol))
}

fn typed_proof_fact_label(
    program: &Program,
    fact: omega_core::arena::Handle<omega_typed_trees::domain::ProofFact>,
) -> String {
    match program.proof_facts.get(fact) {
        omega_typed_trees::domain::ProofFact::Expression(expression) => {
            program.expression_table.display_name(*expression)
        }
        omega_typed_trees::domain::ProofFact::Membership(membership) => {
            let domain = program
                .domain_path_members(membership.domain)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            format!(
                "{} in {}",
                program.expression_table.display_name(membership.value),
                domain
            )
        }
    }
}

fn state_label_from_symbol(program: &Program, symbol: SymbolHandle) -> String {
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
        .unwrap_or_else(|| symbol_label(symbol))
}

fn append_flow_context_labels(
    program: &Program,
    report: &mut String,
    contexts: omega_core::arena::HandleSpan<omega_checked_trees::FlowSemanticContextRef>,
) {
    let contexts = program
        .facts
        .flow
        .semantic_context_refs
        .span_or_empty(contexts);
    if contexts.is_empty() {
        report.push_str("<none>");
        return;
    }

    for (index, context_ref) in contexts.iter().enumerate() {
        if index > 0 {
            report.push_str(" | ");
        }
        let context = program.facts.semantic.contexts.get(context_ref.context);
        report.push_str(&semantic_point_label(program, context.point));
    }
}

fn append_contract_fact_ref_summary(
    program: &Program,
    report: &mut String,
    facts: omega_core::arena::HandleSpan<omega_checked_trees::ContractProofFactRef>,
) {
    let fact_refs = program.facts.proof.contract_fact_refs.span_or_empty(facts);
    if fact_refs.is_empty() {
        report.push_str("<none>");
        return;
    }

    for (index, fact_ref) in fact_refs.iter().enumerate() {
        if index > 0 {
            report.push_str(" | ");
        }
        let fact = program.facts.proof.contract_facts.get(fact_ref.fact);
        report.push_str(&typed_proof_fact_label(program, fact.fact));
    }
}

fn append_contract_fact_ref_list(
    program: &Program,
    report: &mut String,
    label: &str,
    facts: omega_core::arena::HandleSpan<omega_checked_trees::ContractProofFactRef>,
) {
    report.push_str("  ");
    report.push_str(label);
    report.push_str(": ");

    let fact_refs = program.facts.proof.contract_fact_refs.span_or_empty(facts);
    if fact_refs.is_empty() {
        report.push_str("<none>\n");
        return;
    }

    for (index, fact_ref) in fact_refs.iter().enumerate() {
        if index > 0 {
            report.push_str(" | ");
        }
        let fact = program.facts.proof.contract_facts.get(fact_ref.fact);
        report.push_str(&typed_proof_fact_label(program, fact.fact));
    }
    report.push('\n');
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

fn symbol_label(symbol: SymbolHandle) -> String {
    if symbol.is_valid() {
        format!("#{}", symbol.arena_index())
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
            ch if ch.is_control() => output.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => output.push(ch),
        }
    }
    output.push('"');
}
