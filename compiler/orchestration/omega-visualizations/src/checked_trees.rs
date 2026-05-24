use omega_checked_trees::CheckedTrees;
use omega_core::symbols::SymbolHandle;
use omega_effects::EffectSet;

pub fn checked_trees_html(program: &CheckedTrees) -> String {
    crate::phase_diagram::text_report_html("checked_trees", &checked_effects_report(program))
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
    json.push_str("]\n}\n");
    json
}

fn checked_effects_report(program: &CheckedTrees) -> String {
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

    report.push_str("Domain Dependencies\n");
    report.push_str("-------------------\n");
    report.push_str("Packed dependency paths for domain facts used by flow invalidation and future proof analysis.\n\n");
    report.push_str("domains: ");
    report.push_str(&program.facts.domains.dependencies.len().to_string());
    report.push('\n');
    report.push_str("paths:   ");
    report.push_str(&program.facts.domains.dependency_paths.len().to_string());
    report.push('\n');
    report.push_str("segments:");
    report.push(' ');
    report.push_str(&program.facts.domains.segments.len().to_string());
    report.push_str("\n\n");

    for (_, dependency) in program.facts.domains.dependencies.iter() {
        report.push_str("domain ");
        report.push_str(&semantic_symbol_name(program, dependency.domain_symbol));
        report.push('\n');
        let mut path_count = 0usize;
        for path in program.facts.domains.dependency_paths(dependency) {
            path_count = path_count.saturating_add(1);
            report.push_str("  path ");
            report.push_str(&path_count.to_string());
            report.push_str(": ");
            if path.is_empty() {
                report.push_str("<self>");
            } else {
                append_dependency_path_label(program, &mut report, path);
            }
            report.push('\n');
        }
    }

    if !program.facts.domains.dependencies.is_empty() {
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
    report.push_str("statements: ");
    report.push_str(&program.facts.flow.statements.len().to_string());
    report.push('\n');
    report.push_str("exits:    ");
    report.push_str(&program.facts.flow.exits.len().to_string());
    report.push('\n');
    report.push_str("invalidations: ");
    report.push_str(&program.facts.flow.invalidations.len().to_string());
    report.push('\n');
    report.push_str("borrow activations: ");
    report.push_str(&program.facts.flow.borrow_activations.len().to_string());
    report.push('\n');
    report.push_str("borrow weakenings: ");
    report.push_str(&program.facts.flow.borrow_weakenings.len().to_string());
    report.push('\n');
    report.push_str("contexts: ");
    report.push_str(&program.facts.flow.semantic_context_refs.len().to_string());
    report.push('\n');
    report.push_str("constraints: ");
    report.push_str(&program.facts.flow.constraint_refs.len().to_string());
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
        report.push_str("  entry contexts: ");
        append_flow_context_labels(program, &mut report, state_flow.entry_semantic_contexts);
        report.push('\n');
        report.push_str("  entry constraints: ");
        append_flow_constraint_labels(program, &mut report, state_flow.entry_constraints);
        report.push('\n');
        report.push_str("  active borrow loans: ");
        append_flow_borrow_loan_summary(program, &mut report, state_flow.entry_constraints);
        report.push('\n');
        report.push_str("  invalidations: ");
        append_flow_invalidation_summary(program, &mut report, state_flow.invalidations);
        report.push('\n');
        report.push_str("  borrow activations: ");
        append_flow_borrow_activation_summary(program, &mut report, state_flow.borrow_activations);
        report.push('\n');
        report.push_str("  borrow weakenings: ");
        append_flow_borrow_weakening_summary(program, &mut report, state_flow.borrow_weakenings);
        report.push('\n');
        report.push_str("  statements: ");
        report.push_str(
            &program
                .facts
                .flow
                .statements
                .span_or_empty(state_flow.statements)
                .len()
                .to_string(),
        );
        report.push('\n');
        report.push_str("  direct effects:  ");
        report.push_str(&format_effect_set(state_flow.direct_effects));
        report.push('\n');
        report.push_str("  reached effects: ");
        report.push_str(&format_effect_set(state_flow.transitive_effects));
        report.push('\n');

        for statement_flow in program
            .facts
            .flow
            .statements
            .span_or_empty(state_flow.statements)
        {
            report.push_str("  statement ");
            report.push_str(&statement_flow.statement_index.to_string());
            report.push('\n');
            report.push_str("    entry contexts: ");
            append_flow_context_labels(program, &mut report, statement_flow.entry_semantic_contexts);
            report.push('\n');
            report.push_str("    active borrow loans: ");
            append_flow_borrow_loan_summary(program, &mut report, statement_flow.entry_constraints);
            report.push('\n');
        }

        for call_flow in program.facts.flow.calls.span_or_empty(state_flow.calls) {
            report.push_str("  call ");
            report.push_str(&call_flow.statement_index.to_string());
            report.push('.');
            report.push_str(&call_flow.call_ordinal.to_string());
            report.push_str(" -> ");
            report.push_str(&state_label_from_symbol(program, call_flow.target_symbol));
            report.push('\n');
            report.push_str("    entry contexts: ");
            append_flow_context_labels(program, &mut report, call_flow.entry_semantic_contexts);
            report.push('\n');
            report.push_str("    entry constraints: ");
            append_flow_constraint_labels(program, &mut report, call_flow.entry_constraints);
            report.push('\n');
            report.push_str("    active borrow loans: ");
            append_flow_borrow_loan_summary(program, &mut report, call_flow.entry_constraints);
            report.push('\n');
            report.push_str("    borrow accesses: ");
            append_flow_borrow_access_summary(program, &mut report, call_flow.entry_constraints);
            report.push('\n');
            report.push_str("    requires contexts: ");
            append_flow_context_labels(program, &mut report, call_flow.requires_contexts);
            report.push('\n');
            report.push_str("    requires constraints: ");
            append_flow_constraint_labels(program, &mut report, call_flow.requires_constraints);
            report.push('\n');
            report.push_str("    exit contexts: ");
            append_flow_context_labels(program, &mut report, call_flow.exit_semantic_contexts);
            report.push('\n');
            report.push_str("    exit constraints: ");
            append_flow_constraint_labels(program, &mut report, call_flow.exit_constraints);
            report.push('\n');
            report.push_str("    invalidations: ");
            append_flow_invalidation_summary(program, &mut report, call_flow.invalidations);
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
            report.push_str("    entry contexts: ");
            append_flow_context_labels(program, &mut report, exit_flow.entry_semantic_contexts);
            report.push('\n');
            report.push_str("    entry constraints: ");
            append_flow_constraint_labels(program, &mut report, exit_flow.entry_constraints);
            report.push('\n');
            report.push_str("    ensures contexts: ");
            append_flow_context_labels(program, &mut report, exit_flow.ensures_contexts);
            report.push('\n');
            report.push_str("    ensures constraints: ");
            append_flow_constraint_labels(program, &mut report, exit_flow.ensures_constraints);
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

fn semantic_point_label(program: &CheckedTrees, point: omega_facts::ProgramPoint) -> String {
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
        omega_facts::ProgramPoint::CallRequires {
            machine_symbol,
            state_symbol,
            statement_index,
            call_ordinal,
        } => format!(
            "call requires {}::{} {statement_index}.{call_ordinal}",
            machine_name(program, machine_symbol),
            state_name(program, state_symbol)
        ),
        omega_facts::ProgramPoint::CallEnsures {
            machine_symbol,
            state_symbol,
            statement_index,
            call_ordinal,
        } => format!(
            "call ensures {}::{} {statement_index}.{call_ordinal}",
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

fn semantic_fact_label(program: &CheckedTrees, fact: &omega_facts::Fact) -> String {
    let origin = semantic_origin_label(program, fact.origin);
    let payload = semantic_payload_label(program, fact.payload);
    let place = semantic_place_label(program, fact.place);
    format!("{payload} | place: {place} | origin: {origin}")
}

fn semantic_payload_label(program: &CheckedTrees, payload: omega_facts::FactPayload) -> String {
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

fn semantic_place_label(program: &CheckedTrees, place: omega_facts::FactPlace) -> String {
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

fn semantic_canonical_place_label(program: &CheckedTrees, place: &omega_facts::Place) -> String {
    canonical_place_label_from_parts(
        program,
        place.root,
        program
            .facts
            .semantic
            .place_segments
            .span_or_empty(place.segments),
    )
}

fn canonical_place_label_from_parts(
    program: &CheckedTrees,
    root: omega_facts::PlaceRoot,
    segments: &[omega_facts::PlaceSegment],
) -> String {
    let mut label = match root {
        omega_facts::PlaceRoot::Unknown => "unknown".to_owned(),
        omega_facts::PlaceRoot::Symbol(symbol) => semantic_symbol_name(program, symbol),
        omega_facts::PlaceRoot::Expression(expression) => {
            program.expression_table.display_name(expression)
        }
        omega_facts::PlaceRoot::TypeReference(type_reference) => {
            program.display_type_reference(type_reference)
        }
    };

    for segment in segments {
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

fn joined_place_label(
    program: &CheckedTrees,
    semantic: &omega_facts::FactPlan,
    place: &omega_facts::Place,
    extra_segments: &[omega_facts::PlaceSegment],
) -> String {
    let mut segments: Vec<_> = semantic
        .place_segments
        .span_or_empty(place.segments)
        .iter()
        .copied()
        .collect();
    segments.extend(extra_segments.iter().copied());
    canonical_place_label_from_parts(program, place.root, &segments)
}

fn append_dependency_path_label(
    program: &CheckedTrees,
    report: &mut String,
    segments: &[omega_facts::PlaceSegment],
) {
    report.push_str("self");
    for segment in segments {
        match segment {
            omega_facts::PlaceSegment::Field { symbol } => {
                report.push('.');
                report.push_str(&semantic_symbol_name(program, *symbol));
            }
            omega_facts::PlaceSegment::Index { expression } => {
                report.push('[');
                report.push_str(&program.expression_table.display_name(*expression));
                report.push(']');
            }
        }
    }
}

fn semantic_origin_label(program: &CheckedTrees, origin: omega_facts::FactOrigin) -> String {
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

fn semantic_symbol_name(program: &CheckedTrees, symbol: SymbolHandle) -> String {
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

    report
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EntryCapabilityManifest {
    entry_machine: String,
    entry_state: String,
    effects: EffectSet,
}

fn entry_capability_manifest(program: &CheckedTrees) -> EntryCapabilityManifest {
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

fn machine_name(program: &CheckedTrees, symbol: SymbolHandle) -> String {
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol)
        .map(|machine| machine.name.as_str().to_owned())
        .unwrap_or_else(|| symbol_label(symbol))
}

fn state_name(program: &CheckedTrees, symbol: SymbolHandle) -> String {
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

fn contract_fact_owner(program: &CheckedTrees, fact: &omega_checked_trees::ContractProofFact) -> String {
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

fn signature_owner_name(program: &CheckedTrees, symbol: SymbolHandle) -> String {
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

fn state_signature_name(program: &CheckedTrees, symbol: SymbolHandle) -> String {
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
    program: &CheckedTrees,
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
        .unwrap_or_else(|| symbol_label(symbol))
}

fn append_flow_context_labels(
    program: &CheckedTrees,
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

fn append_flow_constraint_labels(
    program: &CheckedTrees,
    report: &mut String,
    constraints: omega_core::arena::HandleSpan<omega_checked_trees::FlowConstraintRef>,
) {
    let constraints = program.facts.flow.constraints(constraints);
    if constraints.is_empty() {
        report.push_str("<none>");
        return;
    }

    for (index, constraint_ref) in constraints.iter().enumerate() {
        if index > 0 {
            report.push_str(" | ");
        }
        match constraint_ref.kind {
            omega_checked_trees::FlowConstraintKind::Unknown => report.push_str("unknown"),
            omega_checked_trees::FlowConstraintKind::SemanticContext { context } => {
                let context = program.facts.semantic.contexts.get(context);
                report.push_str("semantic(");
                report.push_str(&semantic_point_label(program, context.point));
                report.push(')');
            }
            omega_checked_trees::FlowConstraintKind::BorrowState { state } => {
                let state = program.facts.borrow.states.get(state);
                report.push_str("borrow-state(");
                report.push_str(&machine_name(program, state.machine_symbol));
                report.push_str("::");
                report.push_str(&state_name(program, state.state_symbol));
                report.push(')');
            }
            omega_checked_trees::FlowConstraintKind::BorrowCall { call } => {
                let call = program.facts.borrow.calls.get(call);
                report.push_str("borrow-call(");
                report.push_str(&call.statement_index.to_string());
                report.push('.');
                report.push_str(&call.call_ordinal.to_string());
                report.push_str(" -> ");
                report.push_str(&state_label_from_symbol(program, call.target_symbol));
                report.push(')');
            }
            omega_checked_trees::FlowConstraintKind::BorrowWritableRoot { root } => {
                let root = program.facts.borrow.writable_roots.get(root);
                report.push_str("borrow-root(");
                report.push_str(&symbol_label(root.symbol));
                report.push(')');
            }
            omega_checked_trees::FlowConstraintKind::BorrowAccess { access } => {
                let access = program.facts.borrow.argument_accesses.get(access);
                report.push_str("borrow-access(");
                report.push_str(&symbol_label(access.root_symbol));
                for segment in program.facts.borrow.access_segments(access) {
                    match segment {
                        omega_facts::PlaceSegment::Field { symbol } => {
                            report.push('.');
                            report.push_str(&symbol_label(*symbol));
                        }
                        omega_facts::PlaceSegment::Index { expression } => {
                            report.push('[');
                            report.push_str(&expression.arena_index().to_string());
                            report.push(']');
                        }
                    }
                }
                report.push_str(", ");
                report.push_str(match access.kind {
                    omega_checked_trees::BorrowAccessKind::Read => "read",
                    omega_checked_trees::BorrowAccessKind::Mutable => "mutable",
                });
                report.push(')');
            }
            omega_checked_trees::FlowConstraintKind::BorrowLoan { loan } => {
                let loan = program.facts.borrow.loans.get(loan);
                report.push_str("borrow-loan(");
                report.push_str(&symbol_label(loan.owner_symbol));
                report.push_str(" -> ");
                report.push_str(&symbol_label(loan.root_symbol));
                for segment in program.facts.borrow.loan_segments(loan) {
                    match segment {
                        omega_facts::PlaceSegment::Field { symbol } => {
                            report.push('.');
                            report.push_str(&symbol_label(*symbol));
                        }
                        omega_facts::PlaceSegment::Index { expression } => {
                            report.push('[');
                            report.push_str(&expression.arena_index().to_string());
                            report.push(']');
                        }
                    }
                }
                report.push(')');
            }
        }
    }
}

fn append_flow_invalidation_summary(
    program: &CheckedTrees,
    report: &mut String,
    invalidations: omega_core::arena::HandleSpan<omega_checked_trees::FlowInvalidationFact>,
) {
    let invalidations = program.facts.flow.invalidations.span_or_empty(invalidations);
    if invalidations.is_empty() {
        report.push_str("<none>");
        return;
    }

    for (index, invalidation) in invalidations.iter().enumerate() {
        if index > 0 {
            report.push_str(" | ");
        }
        report.push_str(&flow_invalidation_label(program, invalidation));
    }
}

fn append_flow_borrow_loan_summary(
    program: &CheckedTrees,
    report: &mut String,
    constraints: omega_core::arena::HandleSpan<omega_checked_trees::FlowConstraintRef>,
) {
    let loans: Vec<_> = program.facts.flow.borrow_loan_constraints(constraints).collect();
    if loans.is_empty() {
        report.push_str("<none>");
        return;
    }

    for (index, loan_handle) in loans.iter().enumerate() {
        if index > 0 {
            report.push_str(" | ");
        }
        let loan = program.facts.borrow.loans.get(*loan_handle);
        report.push_str(&borrow_loan_label(program, loan));
    }
}

fn append_flow_borrow_activation_summary(
    program: &CheckedTrees,
    report: &mut String,
    activations: omega_core::arena::HandleSpan<omega_checked_trees::FlowBorrowActivationFact>,
) {
    let activations = program.facts.flow.borrow_activations.span_or_empty(activations);
    if activations.is_empty() {
        report.push_str("<none>");
        return;
    }

    for (index, activation) in activations.iter().enumerate() {
        if index > 0 {
            report.push_str(" | ");
        }
        report.push_str(&flow_borrow_activation_label(program, activation));
    }
}

fn append_flow_borrow_access_summary(
    program: &CheckedTrees,
    report: &mut String,
    constraints: omega_core::arena::HandleSpan<omega_checked_trees::FlowConstraintRef>,
) {
    let accesses: Vec<_> = program.facts.flow.borrow_access_constraints(constraints).collect();
    if accesses.is_empty() {
        report.push_str("<none>");
        return;
    }

    for (index, access_handle) in accesses.iter().enumerate() {
        if index > 0 {
            report.push_str(" | ");
        }
        let access = program.facts.borrow.argument_accesses.get(*access_handle);
        report.push_str(&borrow_access_fact_label(program, access));
    }
}

fn append_flow_borrow_weakening_summary(
    program: &CheckedTrees,
    report: &mut String,
    weakenings: omega_core::arena::HandleSpan<omega_checked_trees::FlowBorrowWeakeningFact>,
) {
    let weakenings = program.facts.flow.borrow_weakenings.span_or_empty(weakenings);
    if weakenings.is_empty() {
        report.push_str("<none>");
        return;
    }

    for (index, weakening) in weakenings.iter().enumerate() {
        if index > 0 {
            report.push_str(" | ");
        }
        report.push_str(&flow_borrow_weakening_label(program, weakening));
    }
}

fn append_contract_fact_ref_summary(
    program: &CheckedTrees,
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
    program: &CheckedTrees,
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

fn flow_invalidation_label(
    program: &CheckedTrees,
    invalidation: &omega_checked_trees::FlowInvalidationFact,
) -> String {
    let fact = program.facts.semantic.facts.get(invalidation.fact);
    let mutated = canonical_place_label_from_parts(
        program,
        invalidation.mutated_root,
        program
            .facts
            .flow
            .invalidation_segments
            .span_or_empty(invalidation.mutated_segments),
    );
    let dependency_segments = program
        .facts
        .flow
        .invalidation_segments
        .span_or_empty(invalidation.dependency_segments);
    let invalidated = match fact.place {
        omega_facts::FactPlace::Place(place) => joined_place_label(
            program,
            &program.facts.semantic,
            program.facts.semantic.places.get(place),
            dependency_segments,
        ),
        _ => semantic_place_label(program, fact.place),
    };
    format!(
        "{} invalidated {} by mutating {}",
        flow_invalidation_source_label(program, invalidation.source),
        semantic_payload_label(program, fact.payload),
        mutated
    ) + &format!(" ({invalidated})")
}

fn flow_borrow_activation_label(
    program: &CheckedTrees,
    activation: &omega_checked_trees::FlowBorrowActivationFact,
) -> String {
    let loan = program.facts.borrow.loans.get(activation.loan);
    format!(
        "{} activated local borrow {}",
        flow_invalidation_source_label(program, activation.source),
        borrow_loan_label(program, loan),
    )
}

fn flow_borrow_weakening_label(
    program: &CheckedTrees,
    weakening: &omega_checked_trees::FlowBorrowWeakeningFact,
) -> String {
    let loan = program.facts.borrow.loans.get(weakening.loan);
    let mut label = String::new();
    label.push_str(&flow_invalidation_source_label(program, weakening.source));
    label.push_str(" expired local borrow ");
    label.push_str(&borrow_loan_label(program, loan));
    match weakening.reason {
        omega_checked_trees::FlowBorrowWeakeningReason::LastUseExpired => {
            label.push_str(" (after last use)");
        }
        omega_checked_trees::FlowBorrowWeakeningReason::StateExit => {
            label.push_str(" (released at state exit)");
        }
    }
    label
}

fn borrow_access_fact_label(
    program: &CheckedTrees,
    access: &omega_checked_trees::BorrowArgumentAccessFact,
) -> String {
    let mut label = String::new();
    label.push_str(&symbol_label(access.root_symbol));
    for segment in program.facts.borrow.access_segments(access) {
        match segment {
            omega_facts::PlaceSegment::Field { symbol } => {
                label.push('.');
                label.push_str(&symbol_label(*symbol));
            }
            omega_facts::PlaceSegment::Index { expression } => {
                label.push('[');
                label.push_str(&expression.arena_index().to_string());
                label.push(']');
            }
        }
    }
    label.push_str(": ");
    label.push_str(match access.kind {
        omega_checked_trees::BorrowAccessKind::Read => "read",
        omega_checked_trees::BorrowAccessKind::Mutable => "mutable",
    });
    label
}

fn borrow_loan_label(
    program: &CheckedTrees,
    loan: &omega_checked_trees::BorrowLoanFact,
) -> String {
    let mut label = String::new();
    label.push_str(&symbol_label(loan.owner_symbol));
    label.push_str(" -> ");
    label.push_str(&symbol_label(loan.root_symbol));
    for segment in program.facts.borrow.loan_segments(loan) {
        match segment {
            omega_facts::PlaceSegment::Field { symbol } => {
                label.push('.');
                label.push_str(&symbol_label(*symbol));
            }
            omega_facts::PlaceSegment::Index { expression } => {
                label.push('[');
                label.push_str(&expression.arena_index().to_string());
                label.push(']');
            }
        }
    }
    label.push_str(" [created ");
    label.push_str(&loan.statement_index.to_string());
    label.push_str(", last use ");
    label.push_str(&loan.last_use_statement_index.to_string());
    label.push(']');
    label
}

fn flow_invalidation_source_label(
    program: &CheckedTrees,
    source: omega_checked_trees::FlowInvalidationSource,
) -> String {
    match source {
        omega_checked_trees::FlowInvalidationSource::Statement { statement_index } => {
            format!("statement {statement_index}")
        }
        omega_checked_trees::FlowInvalidationSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => format!(
            "call {statement_index}.{call_ordinal} -> {}",
            state_label_from_symbol(program, target_symbol)
        ),
    }
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
