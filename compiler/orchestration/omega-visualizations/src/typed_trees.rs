use crate::phase_diagram::PhaseDiagramBuilder;
use omega_core::symbols::SymbolHandle;
use omega_effects::{EffectPlan, EffectSet};
use omega_typed_trees::statement::TableNamePath;
use omega_typed_trees::statement::{
    StatementNode, TableCall, TableTransition, TransitionGuardNode, TransitionTargetHandle,
    TransitionTargetNode,
};
use omega_typed_trees::trait_definition::TraitDefinition;
use omega_typed_trees::{TypedTrees, signature::StateSignature, state::State};
use omega_typed_trees::{data::DataMember, machine::Machine};

pub fn typed_trees_html(typed: &TypedTrees) -> String {
    let mut diagram = PhaseDiagramBuilder::new("typed_trees");
    let effect_plan = omega_effects::infer_effects(typed);
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
            format!(
                "domain {} for {}\nsymbol: {}\nbody tokens: {}",
                domain.name.as_str(),
                typed.display_type_reference_with_constraints(domain.target_type),
                symbol_label(domain.symbol),
                domain.body_token_count
            ),
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
                    &effect_plan,
                    machine,
                    Some(root_state),
                    &states,
                    &root_symbols,
                ),
                "machine",
                1,
            );
            if let Some(effects) = machine_effects_for(&effect_plan, machine.symbol) {
                diagram.node_effects(&machine_id, effect_names_from_set(effects.transitive));
            }
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
                &effect_plan,
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
            &effect_plan,
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

    for (object_index, object) in program
        .machine_contained_objects(machine)
        .iter()
        .enumerate()
    {
        let object_id = diagram.node(
            format!("object_{machine_index}_{root_index}_{object_index}"),
            format!(
                "contains {}\ntype: {}\nsymbol: {}\ntype symbol: {}",
                object.name.as_str(),
                object.type_name.as_str(),
                symbol_label(object.symbol),
                symbol_label(object.type_symbol)
            ),
            "object",
            2,
        );
        diagram.containment_edge(machine_id, &object_id);
        if let Some(type_id) = type_id_for_symbol(data_nodes, trait_nodes, object.type_symbol) {
            diagram.edge(&object_id, type_id, "contained_object");
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
    let effects = program.state_signature_effects(machine);
    if !effects.is_empty() {
        label.push_str("\neffects: ");
        label.push_str(
            &effects
                .iter()
                .map(|effect| effect.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    label
}

fn machine_label(
    program: &TypedTrees,
    effect_plan: &EffectPlan,
    machine: &Machine,
    entry_state: Option<&State>,
    states: &[&State],
    root_symbols: &[SymbolHandle],
) -> String {
    let attached_data = machine
        .attached_data
        .as_ref()
        .map(|name| name.as_str())
        .unwrap_or("<none>");
    let mut label = format!(
        "machine {}\nsymbol: {}\nattached data: {}\nsatisfies: {}\ntargetable states: {}",
        machine.name.as_str(),
        symbol_label(machine.symbol),
        attached_data,
        machine.satisfies.len(),
        entry_state
            .map(|entry_state| {
                states
                    .iter()
                    .filter(|state| {
                        state.symbol != entry_state.symbol
                            && root_symbol_for_state(program, states, root_symbols, state.symbol)
                                == entry_state.symbol
                    })
                    .count()
            })
            .unwrap_or(0)
    );
    if let Some(effects) = machine_effects_for(effect_plan, machine.symbol) {
        label.push_str("\ndirect effects: ");
        label.push_str(&format_effect_set(effects.direct));
        label.push_str("\nreached effects: ");
        label.push_str(&format_effect_set(effects.transitive));
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
    effect_plan: &EffectPlan,
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

        let reached_effects = call_effects_for(
            effect_plan,
            state_symbol,
            statement_index,
            call.target_symbol,
        )
        .map(|effects| effects.transitive)
        .unwrap_or_else(EffectSet::empty);
        let call_id = diagram.scoped_node(
            format!(
                "machine_ref_{machine_index}_{}_{}",
                state_symbol.arena_index(),
                statement_index
            ),
            format!(
                "{}\n{}\nreaches effects: {}\n\ndouble-click to scope target",
                call_target.label,
                inline_label(statement_label(program, statement)),
                format_effect_set(reached_effects)
            ),
            "machine_ref",
            2,
            call_target.scope_id,
        );
        diagram.node_effects(&call_id, effect_names_from_set(reached_effects));
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
    effect_plan: &EffectPlan,
    parent_id: &str,
    state_nodes: &[(SymbolHandle, String, String)],
    machine_index: usize,
    state: &State,
) {
    let state_id = diagram.node(
        format!("state_{machine_index}_{}", state.symbol.arena_index()),
        state_label(program, effect_plan, state),
        "state",
        2,
    );
    if let Some(effects) = state_effects_for(effect_plan, state.symbol) {
        diagram.node_effects(&state_id, effect_names_from_set(effects.transitive));
    }
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

fn state_label(program: &TypedTrees, effect_plan: &EffectPlan, state: &State) -> String {
    let mut label = format!(
        "state {}\nsymbol: {}\nparams: {}\nstatements: {}",
        state.name.as_str(),
        symbol_label(state.symbol),
        state.parameters.len(),
        state.statement_nodes.len()
    );
    if let Some(effects) = state_effects_for(effect_plan, state.symbol) {
        label.push_str("\ndirect effects: ");
        label.push_str(&format_effect_set(effects.direct));
        label.push_str("\nreached effects: ");
        label.push_str(&format_effect_set(effects.transitive));
    }

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

fn statement_label(program: &TypedTrees, statement: &StatementNode) -> String {
    match statement {
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
        TransitionTargetNode::Named { path, arguments } => {
            let path = program
                .statement_table
                .name_path_members(path.members)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join(".");
            format!("{path}({})", arguments.len())
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

fn machine_effects_for(
    effect_plan: &EffectPlan,
    symbol: SymbolHandle,
) -> Option<&omega_effects::MachineEffects> {
    effect_plan
        .machines()
        .iter()
        .find(|effects| effects.symbol == symbol)
}

fn state_effects_for(
    effect_plan: &EffectPlan,
    symbol: SymbolHandle,
) -> Option<&omega_effects::StateEffects> {
    effect_plan
        .machines()
        .iter()
        .flat_map(|machine| effect_plan.states.span_or_empty(machine.states).iter())
        .find(|effects| effects.symbol == symbol)
}

fn call_effects_for(
    effect_plan: &EffectPlan,
    state_symbol: SymbolHandle,
    statement_index: usize,
    target_symbol: SymbolHandle,
) -> Option<&omega_effects::CallEffects> {
    let state_effects = state_effects_for(effect_plan, state_symbol)?;
    effect_plan
        .calls
        .span_or_empty(state_effects.calls)
        .iter()
        .find(|effects| {
            effects.statement_index == statement_index
                && effects.target_state_symbol == target_symbol
        })
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

fn effect_names_from_set(effects: EffectSet) -> Vec<String> {
    effects.names().map(str::to_owned).collect()
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
