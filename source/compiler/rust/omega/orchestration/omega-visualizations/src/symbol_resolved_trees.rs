use crate::phase_diagram::PhaseDiagramBuilder;
use psi_arena::HandleSpan;
use psi_symbol_resolved_trees::SymbolResolvedTrees;
use psi_symbol_resolved_trees::data::DataMember;
use psi_symbol_resolved_trees::machine::Machine;
use psi_symbol_resolved_trees::signature::StateSignature;
use psi_symbol_resolved_trees::state::State;
use psi_symbol_resolved_trees::statement::{
    Statement, Transition, TransitionGuard, TransitionTarget,
};
use psi_symbol_resolved_trees::trait_definition::TraitDefinition;
use psi_symbol_resolved_trees::types::TypeReference;
use psi_symbols::SymbolHandle;

pub fn symbol_resolved_trees_html(program: &SymbolResolvedTrees) -> String {
    let mut diagram = PhaseDiagramBuilder::new("symbol_resolved_trees");
    let mut data_nodes: Vec<(SymbolHandle, String, String)> = Vec::new();
    let mut trait_nodes: Vec<(SymbolHandle, String, String)> = Vec::new();

    for (data_index, data) in program.roots.data_definitions.iter().enumerate() {
        let data_id = diagram.node(
            format!("data_{data_index}"),
            format!(
                "data {}\nsymbol: {}\nmembers: {}",
                data.name.as_str(),
                symbol_label(data.symbol),
                data.members.len()
            ),
            "data",
            1,
        );
        data_nodes.push((data.symbol, data_id, data.name.as_str().to_owned()));
    }

    for (trait_index, trait_definition) in program.roots.traits.iter().enumerate() {
        let trait_id = diagram.node(
            format!("trait_{trait_index}"),
            format!(
                "{} {}\nsymbol: {}\nrequires: {}\nmachines: {}",
                if trait_definition.is_boundary {
                    "boundary trait"
                } else {
                    "trait"
                },
                trait_definition.name.as_str(),
                symbol_label(trait_definition.symbol),
                trait_definition.requires.len(),
                trait_definition.machines.len()
            ),
            "trait",
            1,
        );
        trait_nodes.push((
            trait_definition.symbol,
            trait_id.clone(),
            trait_definition.name.as_str().to_owned(),
        ));
        append_trait_machine_signatures(
            &mut diagram,
            program,
            &trait_id,
            trait_index,
            trait_definition,
        );
    }

    for (domain_index, domain) in program.roots.domain_definitions.iter().enumerate() {
        let target_symbol = type_reference_symbol(program, &domain.target_type);
        let domain_id = diagram.node(
            format!("domain_{domain_index}"),
            {
                let mut label = format!(
                    "domain {}\ntarget: {}\nsymbol: {}\nclassification: {}\npredicate body: {}\nfacts: {}\nsemantic clause tokens: {}",
                    domain.name.as_str(),
                    domain
                        .target_type
                        .display_name_with_constraints(&program.tables.types.constraints),
                    symbol_label(domain.symbol),
                    domain.classification.map_or("none", |value| value.as_str()),
                    domain.predicate_body.as_str(),
                    program.proof_facts(domain.facts).len(),
                    domain.semantic_clause_token_count
                );
                for fact in proof_fact_labels(program, domain.facts) {
                    label.push_str("\n  ");
                    label.push_str(&fact);
                }
                label
            },
            "domain",
            1,
        );
        if let Some(target_id) = type_id_for_symbol(&data_nodes, &trait_nodes, target_symbol) {
            diagram.edge(&domain_id, target_id, "domain_target");
        }
    }

    for (trait_index, trait_definition) in program.roots.traits.iter().enumerate() {
        let Some((_, trait_id, _)) = trait_nodes.get(trait_index) else {
            continue;
        };

        append_trait_relationships(
            &mut diagram,
            program,
            &trait_nodes,
            trait_id,
            trait_definition,
        );
    }

    for (data_index, data) in program.roots.data_definitions.iter().enumerate() {
        let Some((_, data_id, _)) = data_nodes.get(data_index) else {
            continue;
        };
        for member in program.data_members(data.members) {
            if let DataMember::Field(field) = member {
                let target_symbol = type_reference_symbol(program, &field.type_reference);
                if let Some(target_id) =
                    type_id_for_symbol(&data_nodes, &trait_nodes, target_symbol)
                {
                    diagram.edge(data_id, target_id, "field_type");
                }
            }
        }
    }

    let mut global_state_scope_nodes = Vec::new();
    let mut global_machine_scope_nodes = Vec::new();
    let mut call_sources = Vec::new();

    for (machine_index, machine) in program.roots.machines.iter().enumerate() {
        let state_handles = program
            .machine_state_handles(machine.states)
            .iter()
            .copied()
            .collect::<Vec<_>>();
        let states = state_handles
            .iter()
            .copied()
            .map(|state_handle| program.machine_state(state_handle))
            .collect::<Vec<_>>();
        let root_symbols = visual_root_symbols(program, &states);
        let mut machine_nodes = Vec::new();
        let mut state_nodes = Vec::new();
        let mut state_scope_nodes = Vec::new();

        for root_symbol in &root_symbols {
            let Some(root_state) = state_by_symbol(&states, *root_symbol) else {
                continue;
            };
            let machine_id = diagram.node(
                format!("machine_{machine_index}_{}", root_symbol.arena_index()),
                machine_label(program, machine, Some(root_state), &states, &root_symbols),
                "machine",
                1,
            );
            append_machine_relationships(
                &mut diagram,
                program,
                &data_nodes,
                &trait_nodes,
                &machine_id,
                machine_index,
                root_symbol.arena_index(),
                machine,
            );
            machine_nodes.push((
                machine.symbol,
                machine.name.as_str().to_owned(),
                root_state.name.as_str().to_owned(),
                *root_symbol,
                machine_id.clone(),
            ));
            global_machine_scope_nodes.push((
                machine.name.as_str().to_owned(),
                root_state.name.as_str().to_owned(),
                machine_id,
            ));
        }

        for state in states.iter().copied() {
            let root_symbol = root_symbol_for_state(program, &states, &root_symbols, state.symbol);
            if let Some(machine_id) =
                machine_id_for_root(&machine_nodes, machine.symbol, root_symbol)
            {
                state_scope_nodes.push((state.symbol, machine_id.to_owned()));
                global_state_scope_nodes.push((state.symbol, machine_id.to_owned()));
            }
        }

        for state in states.iter().copied() {
            let root_symbol = root_symbol_for_state(program, &states, &root_symbols, state.symbol);
            let Some(machine_id) = machine_id_for_root(&machine_nodes, machine.symbol, root_symbol)
            else {
                continue;
            };

            if root_symbols.contains(&state.symbol) {
                call_sources.push((
                    machine_index,
                    machine.name.as_str().to_owned(),
                    state.symbol,
                    machine_id.to_owned(),
                    state,
                ));
                continue;
            }

            let state_id = format!("state_{machine_index}_{}", state.symbol.arena_index());
            state_nodes.push((
                state.symbol,
                state.name.as_str().to_owned(),
                state_id.clone(),
            ));
            call_sources.push((
                machine_index,
                machine.name.as_str().to_owned(),
                state.symbol,
                state_id.clone(),
                state,
            ));
            append_state(
                &mut diagram,
                program,
                machine_id,
                &state_nodes,
                machine_index,
                state,
            );
        }

        for state in states.iter().copied() {
            if root_symbols.contains(&state.symbol) {
                let Some(machine_id) = scope_id_for_state(&state_scope_nodes, state.symbol) else {
                    continue;
                };
                append_entry_transitions(&mut diagram, program, machine_id, &state_nodes, state);
            }
        }
    }

    for (machine_index, source_machine_name, state_symbol, source_id, state) in call_sources {
        append_call_references(
            &mut diagram,
            program,
            &global_state_scope_nodes,
            &global_machine_scope_nodes,
            machine_index,
            &source_machine_name,
            state_symbol,
            &source_id,
            state,
        );
    }

    diagram.finish()
}

fn visual_root_symbols(program: &SymbolResolvedTrees, states: &[&State]) -> Vec<SymbolHandle> {
    let mut incoming = Vec::new();

    for state in states {
        for statement in program.state_statements(state.statements) {
            if let Statement::Transition(transition) = statement {
                for target in [
                    &transition.target,
                    transition
                        .continuation
                        .as_ref()
                        .unwrap_or(&TransitionTarget::Terminal),
                ] {
                    if let Some(target_symbol) = transition_target_symbol_in_states(states, target)
                        && target_symbol != state.symbol
                        && !incoming.contains(&target_symbol)
                    {
                        incoming.push(target_symbol);
                    }
                }
            }
        }
    }

    let mut roots = states
        .iter()
        .filter(|state| !incoming.contains(&state.symbol))
        .map(|state| state.symbol)
        .collect::<Vec<_>>();
    if roots.is_empty() {
        if let Some(first) = states.first() {
            roots.push(first.symbol);
        }
    }
    roots
}

fn transition_target_symbol_in_states(
    states: &[&State],
    target: &TransitionTarget,
) -> Option<SymbolHandle> {
    match target {
        TransitionTarget::Named(named) => states
            .iter()
            .find(|state| state.symbol == named.symbol)
            .map(|state| state.symbol),
        TransitionTarget::Value(_) | TransitionTarget::SelfTarget | TransitionTarget::Terminal => {
            None
        }
    }
}

fn root_symbol_for_state(
    program: &SymbolResolvedTrees,
    states: &[&State],
    root_symbols: &[SymbolHandle],
    state_symbol: SymbolHandle,
) -> SymbolHandle {
    if root_symbols.contains(&state_symbol) {
        return state_symbol;
    }

    for root_symbol in root_symbols {
        if reaches_state(program, states, *root_symbol, state_symbol) {
            return *root_symbol;
        }
    }

    root_symbols.first().copied().unwrap_or(state_symbol)
}

fn reaches_state(
    program: &SymbolResolvedTrees,
    states: &[&State],
    root_symbol: SymbolHandle,
    target_symbol: SymbolHandle,
) -> bool {
    let mut stack = vec![root_symbol];
    let mut visited = Vec::new();

    while let Some(symbol) = stack.pop() {
        if symbol == target_symbol {
            return true;
        }
        if visited.contains(&symbol) {
            continue;
        }
        visited.push(symbol);

        let Some(state) = state_by_symbol(states, symbol) else {
            continue;
        };
        for statement in program.state_statements(state.statements) {
            if let Statement::Transition(transition) = statement {
                if let Some(next_symbol) =
                    transition_target_symbol_in_states(states, &transition.target)
                {
                    stack.push(next_symbol);
                }
                if let Some(continuation) = &transition.continuation
                    && let Some(next_symbol) =
                        transition_target_symbol_in_states(states, continuation)
                {
                    stack.push(next_symbol);
                }
            }
        }
    }

    false
}

fn state_by_symbol<'states>(
    states: &'states [&State],
    symbol: SymbolHandle,
) -> Option<&'states State> {
    states.iter().copied().find(|state| state.symbol == symbol)
}

fn append_trait_relationships(
    diagram: &mut PhaseDiagramBuilder,
    program: &SymbolResolvedTrees,
    trait_nodes: &[(SymbolHandle, String, String)],
    trait_id: &str,
    trait_definition: &TraitDefinition,
) {
    for requirement in program.trait_requirements(trait_definition.requires) {
        if let Some(required_trait_id) = trait_id_for_symbol(trait_nodes, requirement.symbol) {
            diagram.edge(trait_id, required_trait_id, "requires_trait");
        }
    }
}

fn append_machine_relationships(
    diagram: &mut PhaseDiagramBuilder,
    program: &SymbolResolvedTrees,
    data_nodes: &[(SymbolHandle, String, String)],
    trait_nodes: &[(SymbolHandle, String, String)],
    machine_id: &str,
    machine_index: usize,
    root_index: u32,
    machine: &Machine,
) {
    let attached_data_name = machine
        .attached_data
        .as_ref()
        .map(|name| name.as_str())
        .unwrap_or_else(|| machine.name.as_str());
    if let Some(data_id) = data_id_for_name(data_nodes, attached_data_name) {
        diagram.edge(data_id, machine_id, "implements_data");
    }

    for conformance in program.machine_trait_conformances(machine.satisfies) {
        if let Some(trait_id) = trait_id_for_symbol(trait_nodes, conformance.symbol) {
            diagram.edge(machine_id, trait_id, "satisfies_trait");
        }
    }

    for (owned_index, owned) in program
        .machine_owned_data(machine.owned_data)
        .iter()
        .enumerate()
    {
        let object_id = diagram.node(
            format!("owned_{machine_index}_{root_index}_{owned_index}"),
            format!(
                "owns {}\ntype: {}\nsymbol: {}",
                owned.name.as_str(),
                owned
                    .type_reference
                    .display_name_with_constraints(&program.tables.types.constraints),
                symbol_label(owned.symbol)
            ),
            "object",
            2,
        );
        diagram.containment_edge(machine_id, &object_id);
        let target_symbol = type_reference_symbol(program, &owned.type_reference);
        if let Some(type_id) = type_id_for_symbol(data_nodes, trait_nodes, target_symbol) {
            diagram.edge(&object_id, type_id, "owned_data");
        }
    }
}

fn append_trait_machine_signatures(
    diagram: &mut PhaseDiagramBuilder,
    program: &SymbolResolvedTrees,
    trait_id: &str,
    trait_index: usize,
    trait_definition: &TraitDefinition,
) {
    for (machine_index, machine) in program
        .trait_machine_signatures(trait_definition.machines)
        .iter()
        .enumerate()
    {
        let machine_id = diagram.node(
            format!("trait_{trait_index}_machine_{machine_index}"),
            trait_machine_signature_label(program, machine),
            "state",
            2,
        );
        diagram.containment_edge(trait_id, &machine_id);
    }
}

fn trait_machine_signature_label(
    program: &SymbolResolvedTrees,
    machine: &StateSignature,
) -> String {
    let mut label = format!(
        "machine {}\nsymbol: {}\nparams: {}",
        machine.name.as_str(),
        symbol_label(machine.symbol),
        machine.parameters.len()
    );
    let service_reaches = program
        .service_reach_rows
        .services(machine.service_reach_row)
        .iter()
        .map(|service| {
            program
                .service_reaches
                .definition(*service)
                .expect("normalized signature service row references a registered service")
        })
        .collect::<Vec<_>>();
    if !service_reaches.is_empty() {
        label.push_str("\nreaches: ");
        label.push_str(
            &service_reaches
                .iter()
                .map(|service| service.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    if machine.contracts.len() > 0 {
        label.push_str("\ncontracts: ");
        label.push_str(&contract_summary(program, machine.contracts));
        append_contract_facts(&mut label, program, machine.contracts);
    }
    label
}

fn machine_label(
    program: &SymbolResolvedTrees,
    machine: &Machine,
    entry_state: Option<&State>,
    _states: &[&State],
    _root_symbols: &[SymbolHandle],
) -> String {
    let attached_data = machine
        .attached_data
        .as_ref()
        .map(|name| name.as_str())
        .unwrap_or("<none>");
    let mut label = format!(
        "machine {}\nsymbol: {}\nattached data: {}\nsatisfies: {}",
        machine.name.as_str(),
        symbol_label(machine.symbol),
        attached_data,
        machine.satisfies.len(),
    );
    let service_reaches = program
        .service_reach_rows
        .services(machine.service_reach_row)
        .iter()
        .map(|service| {
            program
                .service_reaches
                .definition(*service)
                .expect("normalized machine service row references a registered service")
        })
        .collect::<Vec<_>>();
    if !service_reaches.is_empty() {
        label.push_str("\nreaches: ");
        label.push_str(
            &service_reaches
                .iter()
                .map(|service| service.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    if machine.contracts.len() > 0 {
        label.push_str("\ncontracts: ");
        label.push_str(&contract_summary(program, machine.contracts));
        append_contract_facts(&mut label, program, machine.contracts);
    }

    let Some(entry_state) = entry_state else {
        return label;
    };

    for (statement_index, statement) in program
        .state_statements(entry_state.statements)
        .iter()
        .enumerate()
    {
        label.push('\n');
        label.push_str("  ");
        label.push_str(&statement_index.to_string());
        label.push_str(": ");
        label.push_str(&inline_label(statement_label(program, statement)));
    }

    label
}

fn append_entry_transitions(
    diagram: &mut PhaseDiagramBuilder,
    program: &SymbolResolvedTrees,
    machine_id: &str,
    state_nodes: &[(SymbolHandle, String, String)],
    entry_state: &State,
) {
    for statement in program.state_statements(entry_state.statements) {
        if let Statement::Transition(transition) = statement {
            if let Some(target_id) = transition_target_id(state_nodes, transition) {
                diagram.edge(machine_id, target_id, "transition_target");
            }
        }
    }
}

fn append_call_references(
    diagram: &mut PhaseDiagramBuilder,
    program: &SymbolResolvedTrees,
    state_scope_nodes: &[(SymbolHandle, String)],
    machine_scope_nodes: &[(String, String, String)],
    machine_index: usize,
    source_machine_name: &str,
    state_symbol: SymbolHandle,
    source_id: &str,
    state: &State,
) {
    for (statement_index, statement) in program
        .state_statements(state.statements)
        .iter()
        .enumerate()
    {
        let Statement::Call(call) = statement else {
            continue;
        };
        let Some(call_target) = call_scope_target(
            program,
            state_scope_nodes,
            machine_scope_nodes,
            source_machine_name,
            call,
        ) else {
            continue;
        };
        if call_target.scope_id == source_id {
            continue;
        }

        let call_id = diagram.scoped_node(
            format!(
                "machine_ref_{machine_index}_{}_{}",
                state_symbol.arena_index(),
                statement_index
            ),
            format!(
                "{}\n{}\n\ndouble-click to scope target",
                call_target.label,
                inline_label(statement_label(program, statement))
            ),
            "machine_ref",
            2,
            call_target.scope_id,
        );
        diagram.edge(source_id, &call_id, "call");
        diagram.containment_edge(source_id, &call_id);
    }
}

struct CallTarget<'nodes> {
    scope_id: &'nodes str,
    label: String,
}

fn call_scope_target<'nodes>(
    program: &SymbolResolvedTrees,
    state_scope_nodes: &'nodes [(SymbolHandle, String)],
    machine_scope_nodes: &'nodes [(String, String, String)],
    source_machine_name: &str,
    call: &psi_symbol_resolved_trees::statement::Call,
) -> Option<CallTarget<'nodes>> {
    if let Some(scope_id) = scope_id_for_state(state_scope_nodes, call.target_symbol) {
        let label = machine_scope_label_for_id(machine_scope_nodes, scope_id)
            .unwrap_or_else(|| format!("{source_machine_name}::{}", call.target.as_str()));
        return Some(CallTarget { scope_id, label });
    }

    let receiver_name = program
        .tables
        .declarations
        .statement_path_members
        .span_or_empty(call.receiver)
        .last()
        .map(|name| name.as_str())?;
    let receiver_type_name = receiver_type_name(program, source_machine_name, receiver_name)?;
    let scope_id = machine_scope_id_for_name_and_state(
        machine_scope_nodes,
        receiver_type_name,
        call.target.as_str(),
    )?;
    Some(CallTarget {
        scope_id,
        label: format!("{receiver_type_name}::{}", call.target.as_str()),
    })
}

fn machine_scope_label_for_id(
    machine_scope_nodes: &[(String, String, String)],
    scope_id: &str,
) -> Option<String> {
    machine_scope_nodes
        .iter()
        .find(|(_, _, id)| id == scope_id)
        .map(|(machine_name, state_name, _)| machine_scope_label(machine_name, state_name))
}

fn machine_scope_label(machine_name: &str, state_name: &str) -> String {
    if machine_name.contains("::") {
        machine_name.to_owned()
    } else {
        format!("{machine_name}::{state_name}")
    }
}

fn receiver_type_name<'program>(
    program: &'program SymbolResolvedTrees,
    source_machine_name: &'program str,
    receiver_name: &str,
) -> Option<&'program str> {
    if receiver_name == "self" {
        return Some(source_machine_name);
    }

    let data = program
        .roots
        .data_definitions
        .iter()
        .find(|data| data.name.as_str() == source_machine_name)?;
    for member in program.data_members(data.members) {
        let DataMember::Field(field) = member else {
            continue;
        };
        if field.name.as_str() != receiver_name {
            continue;
        }
        let type_symbol = type_reference_symbol(program, &field.type_reference);
        return program
            .roots
            .data_definitions
            .iter()
            .find(|data| data.symbol == type_symbol)
            .map(|data| data.name.as_str());
    }

    None
}

fn machine_scope_id_for_name_and_state<'nodes>(
    machine_scope_nodes: &'nodes [(String, String, String)],
    machine_name: &str,
    state_name: &str,
) -> Option<&'nodes str> {
    let mut matches = machine_scope_nodes
        .iter()
        .filter(|(candidate_machine, candidate_state, _)| {
            candidate_machine == machine_name && candidate_state == state_name
        })
        .map(|(_, _, id)| id.as_str());
    let first = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(first)
}

fn append_state(
    diagram: &mut PhaseDiagramBuilder,
    program: &SymbolResolvedTrees,
    parent_id: &str,
    state_nodes: &[(SymbolHandle, String, String)],
    machine_index: usize,
    state: &State,
) {
    let state_id = diagram.node(
        format!("state_{machine_index}_{}", state.symbol.arena_index()),
        state_label(program, state),
        "state",
        2,
    );
    diagram.containment_edge(parent_id, &state_id);

    for statement in program.state_statements(state.statements) {
        if let Statement::Transition(transition) = statement {
            if let Some(target_id) = transition_target_id(state_nodes, transition) {
                diagram.edge(&state_id, target_id, "transition_target");
            }
        }
    }
}

fn state_label(program: &SymbolResolvedTrees, state: &State) -> String {
    let mut label = format!(
        "state {}\nsymbol: {}\nparams: {}",
        state.name.as_str(),
        symbol_label(state.symbol),
        state.parameters.len()
    );

    for (statement_index, statement) in program
        .state_statements(state.statements)
        .iter()
        .enumerate()
    {
        label.push('\n');
        label.push_str("  ");
        label.push_str(&statement_index.to_string());
        label.push_str(": ");
        label.push_str(&inline_label(statement_label(program, statement)));
    }

    label
}

fn inline_label(label: String) -> String {
    label.replace('\n', " | ")
}

fn contract_summary(
    program: &SymbolResolvedTrees,
    contracts: HandleSpan<psi_symbol_resolved_trees::signature::SignatureContract>,
) -> String {
    if contracts.len() == 0 {
        return "0".to_owned();
    }

    format!(
        "{} / facts: {}",
        contracts.len(),
        contract_fact_count(program, contracts)
    )
}

fn contract_fact_count(
    program: &SymbolResolvedTrees,
    contracts: HandleSpan<psi_symbol_resolved_trees::signature::SignatureContract>,
) -> usize {
    program
        .signature_contracts(contracts)
        .iter()
        .map(|contract| program.proof_facts(contract.facts).len())
        .sum()
}

fn append_contract_facts(
    label: &mut String,
    program: &SymbolResolvedTrees,
    contracts: HandleSpan<psi_symbol_resolved_trees::signature::SignatureContract>,
) {
    for (contract_index, contract) in program.signature_contracts(contracts).iter().enumerate() {
        let facts = proof_fact_labels(program, contract.facts);
        if facts.is_empty() {
            continue;
        }

        label.push_str("\n  contract ");
        label.push_str(&contract_index.to_string());
        label.push_str(": ");
        label.push_str(&facts.join(" | "));
    }
}

fn proof_fact_labels(
    program: &SymbolResolvedTrees,
    facts: HandleSpan<psi_symbol_resolved_trees::domain::ProofFact>,
) -> Vec<String> {
    program
        .proof_facts(facts)
        .iter()
        .take(5)
        .map(|fact| proof_fact_label(program, fact))
        .collect()
}

fn proof_fact_label(
    program: &SymbolResolvedTrees,
    fact: &psi_symbol_resolved_trees::domain::ProofFact,
) -> String {
    match fact {
        psi_symbol_resolved_trees::domain::ProofFact::Expression(expression) => {
            program.tables.bodies.expressions.display_name(*expression)
        }
        psi_symbol_resolved_trees::domain::ProofFact::Membership(membership) => {
            let domain = program
                .domain_path_members(membership.domain)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            format!(
                "{} in {}",
                program
                    .tables
                    .bodies
                    .expressions
                    .display_name(membership.value),
                domain
            )
        }
    }
}

fn statement_label(program: &SymbolResolvedTrees, statement: &Statement) -> String {
    match statement {
        Statement::AssemblyFact(fact) => format!(
            "asm {} {}",
            match fact.kind {
                psi_symbol_resolved_trees::statement::AssemblyFactKind::Requires => "requires",
                psi_symbol_resolved_trees::statement::AssemblyFactKind::Ensures => "ensures",
            },
            program
                .tables
                .bodies
                .expressions
                .display_name(fact.expression),
        ),
        Statement::Assignment(assignment) => format!(
            "{} = {}",
            program
                .tables
                .bodies
                .expressions
                .display_name(assignment.target),
            program
                .tables
                .bodies
                .expressions
                .display_name(assignment.value)
        ),
        Statement::Call(call) => format!(
            "call {}\ntarget: {}",
            call.target.as_str(),
            symbol_label(call.target_symbol)
        ),
        Statement::ProofOutputBindingStatement(package) => format!(
            "evidence package {}",
            package
                .bindings
                .iter()
                .map(|binding| format!(
                    "{}: {}",
                    binding.output_field.as_str(),
                    binding.binding.as_str()
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Statement::Expression(_) => "expression".to_owned(),
        Statement::LocalData(local) => format!(
            "local {}\nsymbol: {}",
            local.name.as_str(),
            symbol_label(local.symbol)
        ),
        Statement::Transition(transition) => transition_label(transition),
    }
}

fn transition_label(transition: &Transition) -> String {
    let guard = match transition.guard {
        TransitionGuard::Always => "always",
        TransitionGuard::When(_) => "when",
    };
    format!(
        "transition {}\nguard: {guard}",
        transition_target_label(&transition.target)
    )
}

fn transition_target_label(target: &TransitionTarget) -> String {
    match target {
        TransitionTarget::Named(named) => {
            format!("state {}", symbol_label(named.symbol))
        }
        TransitionTarget::Value(_) => "value".to_owned(),
        TransitionTarget::SelfTarget => "self".to_owned(),
        TransitionTarget::Terminal => "terminal".to_owned(),
    }
}

fn transition_target_id<'a>(
    state_nodes: &'a [(SymbolHandle, String, String)],
    transition: &Transition,
) -> Option<&'a str> {
    match &transition.target {
        TransitionTarget::Named(named) => state_id_for_symbol(state_nodes, named.symbol),
        TransitionTarget::Value(_) | TransitionTarget::SelfTarget | TransitionTarget::Terminal => {
            None
        }
    }
}

fn symbol_label(symbol: SymbolHandle) -> String {
    if symbol.is_valid() {
        format!("#{}", symbol.arena_index())
    } else {
        "invalid".to_owned()
    }
}

fn type_reference_symbol(
    program: &SymbolResolvedTrees,
    type_reference: &TypeReference,
) -> SymbolHandle {
    match type_reference {
        TypeReference::Reference(reference) => type_reference_symbol(
            program,
            program
                .tables
                .declarations
                .child_type_references
                .get(reference.referee),
        ),
        TypeReference::Constrained(constrained) => type_reference_symbol(
            program,
            program
                .tables
                .declarations
                .child_type_references
                .get(constrained.base_type),
        ),
        TypeReference::FixedArray(fixed_array) => type_reference_symbol(
            program,
            program
                .tables
                .declarations
                .child_type_references
                .get(fixed_array.element_type),
        ),
        TypeReference::Slice(slice) => type_reference_symbol(
            program,
            program
                .tables
                .declarations
                .child_type_references
                .get(slice.element_type),
        ),
        TypeReference::Generic(generic) => generic.base_symbol,
        TypeReference::ConstExpression(_) => SymbolHandle::invalid(),
        TypeReference::DynamicTrait { symbol, .. } => *symbol,
        TypeReference::Named { symbol, .. } | TypeReference::SelfType { symbol } => *symbol,
        TypeReference::Unit => SymbolHandle::invalid(),
    }
}

fn data_id_for_symbol(
    data_nodes: &[(SymbolHandle, String, String)],
    symbol: SymbolHandle,
) -> Option<&str> {
    if !symbol.is_valid() {
        return None;
    }

    data_nodes
        .iter()
        .find(|(data_symbol, _, _)| *data_symbol == symbol)
        .map(|(_, data_id, _)| data_id.as_str())
}

fn type_id_for_symbol<'a>(
    data_nodes: &'a [(SymbolHandle, String, String)],
    trait_nodes: &'a [(SymbolHandle, String, String)],
    symbol: SymbolHandle,
) -> Option<&'a str> {
    data_id_for_symbol(data_nodes, symbol).or_else(|| trait_id_for_symbol(trait_nodes, symbol))
}

fn trait_id_for_symbol(
    trait_nodes: &[(SymbolHandle, String, String)],
    symbol: SymbolHandle,
) -> Option<&str> {
    if !symbol.is_valid() {
        return None;
    }

    trait_nodes
        .iter()
        .find(|(trait_symbol, _, _)| *trait_symbol == symbol)
        .map(|(_, trait_id, _)| trait_id.as_str())
}

fn data_id_for_name<'a>(
    data_nodes: &'a [(SymbolHandle, String, String)],
    name: &str,
) -> Option<&'a str> {
    data_nodes
        .iter()
        .find(|(_, _, data_name)| data_name == name)
        .map(|(_, data_id, _)| data_id.as_str())
}

fn machine_id_for_root(
    machine_nodes: &[(SymbolHandle, String, String, SymbolHandle, String)],
    symbol: SymbolHandle,
    root_symbol: SymbolHandle,
) -> Option<&str> {
    machine_nodes
        .iter()
        .find(|(machine_symbol, _, _, candidate_root_symbol, _)| {
            *machine_symbol == symbol && *candidate_root_symbol == root_symbol
        })
        .map(|(_, _, _, _, id)| id.as_str())
}

fn scope_id_for_state(
    state_scope_nodes: &[(SymbolHandle, String)],
    symbol: SymbolHandle,
) -> Option<&str> {
    state_scope_nodes
        .iter()
        .find(|(candidate, _)| *candidate == symbol)
        .map(|(_, id)| id.as_str())
}

fn state_id_for_symbol(
    state_nodes: &[(SymbolHandle, String, String)],
    symbol: SymbolHandle,
) -> Option<&str> {
    if !symbol.is_valid() {
        return None;
    }

    state_nodes
        .iter()
        .find(|(state_symbol, _, _)| *state_symbol == symbol)
        .map(|(_, _, state_id)| state_id.as_str())
}
