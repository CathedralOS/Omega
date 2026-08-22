mod behavior;

use crate::phase_diagram::PhaseDiagramBuilder;
use crate::service_reach::{append_reach_and_operation_lines, service_names};
use behavior::TypedBehaviorPlan;
use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;
use psi_typed_trees::statement::TableNamePath;
use psi_typed_trees::statement::{
    StatementNode, TableCall, TableTransition, TransitionGuardNode, TransitionTargetHandle,
    TransitionTargetNode,
};
use psi_typed_trees::trait_definition::TraitDefinition;
use psi_typed_trees::{TypedTrees, signature::StateSignature, state::State};
use psi_typed_trees::{data::DataMember, machine::Machine};

pub fn typed_trees_html(typed: &TypedTrees) -> String {
    let mut diagram = PhaseDiagramBuilder::new("typed_trees");
    let behavior = TypedBehaviorPlan::infer(typed);
    let mut data_nodes: Vec<(SymbolHandle, String, String)> = Vec::new();
    let mut trait_nodes: Vec<(SymbolHandle, String, String)> = Vec::new();

    for (data_index, data) in typed.data_definitions().iter().enumerate() {
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

    for (trait_index, trait_definition) in typed.traits().iter().enumerate() {
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
            typed,
            &trait_id,
            trait_index,
            trait_definition,
        );
    }

    for (domain_index, domain) in typed.domain_definitions().iter().enumerate() {
        let target_symbol = typed.type_reference_symbol(domain.target_type);
        let domain_id = diagram.node(
            format!("domain_{domain_index}"),
            {
                let mut label = format!(
                    "domain {}\ntarget: {}\nsymbol: {}\npredicate body: {}\nfacts: {}\nsemantic clause tokens: {}",
                    domain.name.as_str(),
                    typed.display_type_reference_with_constraints(domain.target_type),
                    symbol_label(domain.symbol),
                    domain.predicate_body.as_str(),
                    typed.proof_facts(domain).len(),
                    domain.semantic_clause_token_count
                );
                for fact in proof_fact_labels(typed, domain.facts) {
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

    for (trait_index, trait_definition) in typed.traits().iter().enumerate() {
        let Some((_, trait_id, _)) = trait_nodes.get(trait_index) else {
            continue;
        };

        append_trait_relationships(
            &mut diagram,
            typed,
            &trait_nodes,
            trait_id,
            trait_definition,
        );
    }

    for (data_index, data) in typed.data_definitions().iter().enumerate() {
        let Some((_, data_id, _)) = data_nodes.get(data_index) else {
            continue;
        };
        for member in typed.data_members(data) {
            if let DataMember::Field(field) = member {
                let target_symbol = typed.type_reference_symbol(field.type_reference);
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

    for (machine_index, machine) in typed.machines().iter().enumerate() {
        let states = typed.machine_states(machine).iter().collect::<Vec<_>>();
        let root_symbols = visual_root_symbols(typed, &states);
        let mut machine_nodes = Vec::new();
        let mut state_nodes = Vec::new();
        let mut state_scope_nodes = Vec::new();

        for root_symbol in &root_symbols {
            let Some(root_state) = state_by_symbol(&states, *root_symbol) else {
                continue;
            };
            let machine_id = diagram.node(
                format!("machine_{machine_index}_{}", root_symbol.arena_index()),
                machine_label(
                    typed,
                    &behavior,
                    machine,
                    Some(root_state),
                    &states,
                    &root_symbols,
                ),
                "machine",
                1,
            );
            let (service_reach, _, _) = behavior.machine(machine.symbol);
            diagram.node_service_reaches(
                &machine_id,
                service_names(
                    &typed.service_reaches,
                    behavior.service_rows(),
                    service_reach.transitive,
                ),
            );
            append_machine_relationships(
                &mut diagram,
                typed,
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
            let root_symbol = root_symbol_for_state(typed, &states, &root_symbols, state.symbol);
            if let Some(machine_id) =
                machine_id_for_root(&machine_nodes, machine.symbol, root_symbol)
            {
                state_scope_nodes.push((state.symbol, machine_id.to_owned()));
                global_state_scope_nodes.push((state.symbol, machine_id.to_owned()));
            }
        }

        for state in states.iter().copied() {
            let root_symbol = root_symbol_for_state(typed, &states, &root_symbols, state.symbol);
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
                typed,
                &behavior,
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
                append_entry_transitions(&mut diagram, typed, machine_id, &state_nodes, state);
            }
        }
    }

    for (machine_index, source_machine_name, state_symbol, source_id, state) in call_sources {
        append_call_references(
            &mut diagram,
            typed,
            &behavior,
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

fn visual_root_symbols(program: &TypedTrees, states: &[&State]) -> Vec<SymbolHandle> {
    let mut incoming = Vec::new();

    for state in states {
        for statement in program.statement_table.statements(state.statement_nodes) {
            if let StatementNode::Transition(transition) = statement {
                for target in [transition.target, transition.continuation] {
                    if let Some(target_symbol) =
                        transition_target_symbol_in_states(program, states, target)
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
    program: &TypedTrees,
    states: &[&State],
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

fn root_symbol_for_state(
    program: &TypedTrees,
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
    program: &TypedTrees,
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
        for statement in program.statement_table.statements(state.statement_nodes) {
            if let StatementNode::Transition(transition) = statement {
                for target in [transition.target, transition.continuation] {
                    if let Some(next_symbol) =
                        transition_target_symbol_in_states(program, states, target)
                    {
                        stack.push(next_symbol);
                    }
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
    program: &TypedTrees,
    trait_nodes: &[(SymbolHandle, String, String)],
    trait_id: &str,
    trait_definition: &TraitDefinition,
) {
    for requirement in program.trait_requirements(trait_definition) {
        if let Some(required_trait_id) = trait_id_for_symbol(trait_nodes, requirement.symbol) {
            diagram.edge(trait_id, required_trait_id, "requires_trait");
        }
    }
}

fn append_machine_relationships(
    diagram: &mut PhaseDiagramBuilder,
    program: &TypedTrees,
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

    for conformance in program.machine_trait_conformances(machine) {
        if let Some(trait_id) = trait_id_for_symbol(trait_nodes, conformance.symbol) {
            diagram.edge(machine_id, trait_id, "satisfies_trait");
        }
    }

    for (owned_index, owned) in program.machine_owned_data(machine).iter().enumerate() {
        let object_id = diagram.node(
            format!("owned_{machine_index}_{root_index}_{owned_index}"),
            format!(
                "owns {}\ntype: {}\nsymbol: {}",
                owned.name.as_str(),
                program.display_type_reference(owned.type_reference),
                symbol_label(owned.symbol)
            ),
            "object",
            2,
        );
        diagram.containment_edge(machine_id, &object_id);
        let target_symbol = program.type_reference_symbol(owned.type_reference);
        if let Some(type_id) = type_id_for_symbol(data_nodes, trait_nodes, target_symbol) {
            diagram.edge(&object_id, type_id, "owned_data");
        }
    }
}

fn append_trait_machine_signatures(
    diagram: &mut PhaseDiagramBuilder,
    program: &TypedTrees,
    trait_id: &str,
    trait_index: usize,
    trait_definition: &TraitDefinition,
) {
    for (machine_index, machine) in program
        .trait_machine_signatures(trait_definition)
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

fn trait_machine_signature_label(program: &TypedTrees, machine: &StateSignature) -> String {
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
    program: &TypedTrees,
    behavior: &TypedBehaviorPlan,
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
    let (service_reach, suspension, blocking) = behavior.machine(machine.symbol);
    append_reach_and_operation_lines(
        &mut label,
        &program.service_reaches,
        behavior.service_rows(),
        service_reach,
        suspension,
        blocking,
    );
    if machine.contracts.len() > 0 {
        label.push_str("\ncontracts: ");
        label.push_str(&contract_summary(program, machine.contracts));
        append_contract_facts(&mut label, program, machine.contracts);
    }

    let Some(entry_state) = entry_state else {
        return label;
    };

    for (statement_index, statement) in program
        .statement_table
        .statements(entry_state.statement_nodes)
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
    program: &TypedTrees,
    machine_id: &str,
    state_nodes: &[(SymbolHandle, String, String)],
    entry_state: &State,
) {
    for statement in program
        .statement_table
        .statements(entry_state.statement_nodes)
        .iter()
    {
        if let StatementNode::Transition(transition) = statement {
            if let Some(target_id) = transition_target_id(program, state_nodes, transition) {
                diagram.edge(machine_id, target_id, "transition_target");
            }
        }
    }
}

fn append_call_references(
    diagram: &mut PhaseDiagramBuilder,
    program: &TypedTrees,
    behavior: &TypedBehaviorPlan,
    state_scope_nodes: &[(SymbolHandle, String)],
    machine_scope_nodes: &[(String, String, String)],
    machine_index: usize,
    source_machine_name: &str,
    state_symbol: SymbolHandle,
    source_id: &str,
    state: &State,
) {
    for (statement_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        let StatementNode::Call(call) = statement else {
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

        let (service_reach, suspension, blocking) =
            behavior.call(state_symbol, statement_index, call.target_symbol);
        let mut label = format!(
            "{}\n{}",
            call_target.label,
            inline_label(statement_label(program, statement)),
        );
        append_reach_and_operation_lines(
            &mut label,
            &program.service_reaches,
            behavior.service_rows(),
            service_reach,
            suspension,
            blocking,
        );
        label.push_str("\n\ndouble-click to scope target");
        let call_id = diagram.scoped_node(
            format!(
                "machine_ref_{machine_index}_{}_{}",
                state_symbol.arena_index(),
                statement_index
            ),
            label,
            "machine_ref",
            2,
            call_target.scope_id,
        );
        diagram.node_service_reaches(
            &call_id,
            service_names(
                &program.service_reaches,
                behavior.service_rows(),
                service_reach.transitive,
            ),
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
    program: &TypedTrees,
    state_scope_nodes: &'nodes [(SymbolHandle, String)],
    machine_scope_nodes: &'nodes [(String, String, String)],
    source_machine_name: &str,
    call: &TableCall,
) -> Option<CallTarget<'nodes>> {
    if let Some(scope_id) = scope_id_for_state(state_scope_nodes, call.target_symbol) {
        let label = machine_scope_label_for_id(machine_scope_nodes, scope_id)
            .unwrap_or_else(|| format!("{source_machine_name}::{}", call.target.as_str()));
        return Some(CallTarget { scope_id, label });
    }

    let receiver_name = program
        .statement_table
        .name_path_members(call.receiver)
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
    program: &'program TypedTrees,
    source_machine_name: &'program str,
    receiver_name: &str,
) -> Option<&'program str> {
    if receiver_name == "self" {
        return Some(source_machine_name);
    }

    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == source_machine_name)?;
    for member in program.data_members(data) {
        let DataMember::Field(field) = member else {
            continue;
        };
        if field.name.as_str() != receiver_name {
            continue;
        }
        let type_symbol = program.type_reference_symbol(field.type_reference);
        return program
            .data_definitions()
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
    program: &TypedTrees,
    behavior: &TypedBehaviorPlan,
    parent_id: &str,
    state_nodes: &[(SymbolHandle, String, String)],
    machine_index: usize,
    state: &State,
) {
    let state_id = diagram.node(
        format!("state_{machine_index}_{}", state.symbol.arena_index()),
        state_label(program, behavior, state),
        "state",
        2,
    );
    let (service_reach, _, _) = behavior.state(state.symbol);
    diagram.node_service_reaches(
        &state_id,
        service_names(
            &program.service_reaches,
            behavior.service_rows(),
            service_reach.transitive,
        ),
    );
    diagram.containment_edge(parent_id, &state_id);

    for statement in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
    {
        if let StatementNode::Transition(transition) = statement {
            if let Some(target_id) = transition_target_id(program, state_nodes, transition) {
                diagram.edge(&state_id, target_id, "transition_target");
            }
        }
    }
}

fn state_label(program: &TypedTrees, behavior: &TypedBehaviorPlan, state: &State) -> String {
    let mut label = format!(
        "state {}\nsymbol: {}\nparams: {}",
        state.name.as_str(),
        symbol_label(state.symbol),
        state.parameters.len(),
    );
    let (service_reach, suspension, blocking) = behavior.state(state.symbol);
    append_reach_and_operation_lines(
        &mut label,
        &program.service_reaches,
        behavior.service_rows(),
        service_reach,
        suspension,
        blocking,
    );

    for (statement_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
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
    program: &TypedTrees,
    contracts: HandleSpan<psi_typed_trees::signature::SignatureContract>,
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
    program: &TypedTrees,
    contracts: HandleSpan<psi_typed_trees::signature::SignatureContract>,
) -> usize {
    program
        .signature_contracts
        .span_or_empty(contracts)
        .iter()
        .map(|contract| program.proof_facts.span_or_empty(contract.facts).len())
        .sum()
}

fn append_contract_facts(
    label: &mut String,
    program: &TypedTrees,
    contracts: HandleSpan<psi_typed_trees::signature::SignatureContract>,
) {
    for (contract_index, contract) in program
        .signature_contracts
        .span_or_empty(contracts)
        .iter()
        .enumerate()
    {
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
    program: &TypedTrees,
    facts: HandleSpan<psi_typed_trees::domain::ProofFact>,
) -> Vec<String> {
    program
        .proof_facts
        .span_or_empty(facts)
        .iter()
        .take(5)
        .map(|fact| proof_fact_label(program, fact))
        .collect()
}

fn proof_fact_label(program: &TypedTrees, fact: &psi_typed_trees::domain::ProofFact) -> String {
    match fact {
        psi_typed_trees::domain::ProofFact::Expression(expression) => {
            program.expression_table.display_name(*expression)
        }
        psi_typed_trees::domain::ProofFact::Membership(membership) => {
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
        psi_typed_trees::domain::ProofFact::Proposition(application) => {
            let binders = application
                .binder_arguments
                .iter()
                .map(|argument| {
                    argument
                        .path
                        .iter()
                        .map(|member| member.as_str())
                        .collect::<Vec<_>>()
                        .join("::")
                })
                .collect::<Vec<_>>();
            let arguments = program
                .expression_table
                .expression_handles(application.arguments)
                .iter()
                .map(|argument| program.expression_table.display_name(*argument))
                .collect::<Vec<_>>();
            let binder_suffix = if binders.is_empty() {
                String::new()
            } else {
                format!("[{}]", binders.join(", "))
            };
            format!(
                "{}{binder_suffix}({})",
                application.name.as_str(),
                arguments.join(", ")
            )
        }
    }
}

fn statement_label(program: &TypedTrees, statement: &StatementNode) -> String {
    match statement {
        StatementNode::AssemblyFact(fact) => format!(
            "asm {} {}",
            match fact.kind {
                psi_typed_trees::statement::AssemblyFactKind::Requires => "requires",
                psi_typed_trees::statement::AssemblyFactKind::Ensures => "ensures",
            },
            program.expression_table.display_name(fact.expression),
        ),
        StatementNode::Assignment(assignment) => format!(
            "{} = {}",
            program.expression_table.display_name(assignment.target),
            program.expression_table.display_name(assignment.value)
        ),
        StatementNode::Call(call) => call_label(call),
        StatementNode::Expression(_) => "expression".to_owned(),
        StatementNode::LocalData(local) => format!(
            "local {}: {}\nsymbol: {}",
            local.name.as_str(),
            program.display_type_reference_with_constraints(local.type_reference),
            symbol_label(local.symbol)
        ),
        StatementNode::Transition(transition) => transition_label(program, transition),
    }
}

fn call_label(call: &TableCall) -> String {
    format!(
        "call {}\ntarget: {}\nargs: {}",
        call.target.as_str(),
        symbol_label(call.target_symbol),
        call.arguments.len()
    )
}

fn transition_label(program: &TypedTrees, transition: &TableTransition) -> String {
    let target = transition_target_label(program, transition.target);
    let guard = match transition.guard {
        TransitionGuardNode::Always => "always",
        TransitionGuardNode::When(_) => "when",
    };
    format!("transition {target}\nguard: {guard}")
}

fn transition_target_label(program: &TypedTrees, target: TransitionTargetHandle) -> String {
    if !target.is_valid() {
        return "terminal".to_owned();
    }

    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named {
            path,
            arguments,
            evidence_arguments,
        } => {
            let path = program
                .statement_table
                .name_path_members(path.members)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join(".");
            format!(
                "{path}({}; {} evidence)",
                arguments.len(),
                evidence_arguments.len()
            )
        }
        TransitionTargetNode::Value(_) => "value".to_owned(),
        TransitionTargetNode::SelfTarget => "self".to_owned(),
        TransitionTargetNode::Terminal => "terminal".to_owned(),
    }
}

fn transition_target_id<'a>(
    program: &TypedTrees,
    state_nodes: &'a [(SymbolHandle, String, String)],
    transition: &TableTransition,
) -> Option<&'a str> {
    if !transition.target.is_valid() {
        return None;
    }

    match program.statement_table.transition_target(transition.target) {
        TransitionTargetNode::Named { path, .. } => state_id_for_symbol(state_nodes, path.symbol)
            .or_else(|| state_id_for_name(state_nodes, transition_target_name(program, *path)?)),
        TransitionTargetNode::Value(_)
        | TransitionTargetNode::SelfTarget
        | TransitionTargetNode::Terminal => None,
    }
}

fn transition_target_name(program: &TypedTrees, path: TableNamePath) -> Option<&str> {
    program
        .statement_table
        .name_path_members(path.members)
        .last()
        .map(|member| member.as_str())
}

fn symbol_label(symbol: SymbolHandle) -> String {
    if symbol.is_valid() {
        format!("#{}", symbol.arena_index())
    } else {
        "invalid".to_owned()
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

fn state_id_for_name<'a>(
    state_nodes: &'a [(SymbolHandle, String, String)],
    name: &str,
) -> Option<&'a str> {
    state_nodes
        .iter()
        .find(|(_, state_name, _)| state_name == name)
        .map(|(_, _, state_id)| state_id.as_str())
}
