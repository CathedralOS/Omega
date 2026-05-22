use crate::phase_diagram::PhaseDiagramBuilder;
use omega_core::symbols::SymbolHandle;
use omega_state_graph::{
    MachineGraph, Operation, OperationExpressionRefs, OperationKind, PlannedTransitionTarget,
    StateGraph, StateKey, StateNode, TransitionEdge,
};
use omega_typed_trees::expression::ExpressionHandle;

pub fn state_graph_html(graph: &StateGraph) -> String {
    let mut diagram = PhaseDiagramBuilder::new("state_graph");
    let mut machine_nodes = Vec::new();
    let mut state_nodes = Vec::new();

    for (machine_index, machine) in graph.machines.iter() {
        let machine_id = diagram.node(
            format!("machine_{}", machine_index.arena_index()),
            machine_label(graph, machine),
            "machine",
            machine_index.arena_index() as usize,
        );
        machine_nodes.push((
            machine.symbol,
            machine.name.as_str().to_owned(),
            machine_id.clone(),
        ));
        let mut seen_state_keys = Vec::new();
        for state in graph.states.span_or_empty(machine.states) {
            if seen_state_keys.iter().any(|key| *key == state.key) {
                continue;
            }
            seen_state_keys.push(state.key);

            let state_id = diagram.node(
                format!(
                    "state_{}_{}_{}",
                    machine_index.arena_index(),
                    state.key.state.arena_index(),
                    state.key.segment_index
                ),
                state_label(graph, machine, state),
                "state_block",
                machine_index.arena_index() as usize,
            );
            diagram.containment_edge(&machine_id, &state_id);
            state_nodes.push((state.key, state_id));
        }
    }

    for (_, machine) in graph.machines.iter() {
        let source_machine_id = machine_id_for_symbol(&machine_nodes, machine.symbol);
        for state in graph.states.span_or_empty(machine.states) {
            let Some(source_id) = state_id_for_key(&state_nodes, state.key) else {
                continue;
            };

            for operation in graph.operations.span_or_empty(state.operations) {
                if let Some(target_id) = operation_call_target_id(graph, &state_nodes, operation) {
                    diagram.edge(source_id, target_id, "call");
                } else if let Some(scope_target_id) =
                    operation_external_call_scope_id(graph, &machine_nodes, machine, operation)
                {
                    if Some(scope_target_id) != source_machine_id {
                        let call_id = diagram.scoped_node(
                            format!(
                                "external_call_{}_{}_{}_{}",
                                state.key.machine.arena_index(),
                                state.key.state.arena_index(),
                                state.key.segment_index,
                                operation.statement_index
                            ),
                            format!(
                                "external call\n{}\n\ndouble-click to scope target",
                                operation_label(graph, operation)
                            ),
                            "external_call",
                            machine_index_from_key(state.key),
                            scope_target_id,
                        );
                        diagram.edge(source_id, &call_id, "call");
                    }
                }
            }

            for transition in graph.transitions.span_or_empty(state.transitions) {
                append_transition_edges(&mut diagram, graph, &state_nodes, source_id, transition);
            }
        }
    }

    diagram.finish()
}

fn machine_index_from_key(key: StateKey) -> usize {
    key.machine.arena_index() as usize
}

fn machine_label(graph: &StateGraph, machine: &MachineGraph) -> String {
    format!(
        "machine {}\nstates: {}\ncontains: {}\nowned data: {}",
        machine.name.as_str(),
        graph.states.span_or_empty(machine.states).len(),
        graph.machine_contains(machine).len(),
        graph.machine_owned_data(machine).len()
    )
}

fn state_label(graph: &StateGraph, machine: &MachineGraph, state: &StateNode) -> String {
    let mut label = format!(
        "{}::{} [block {}]\nparams: {}  mutable params: {}\nops: {}  transitions: {}",
        machine.name.as_str(),
        state.name.as_str(),
        state.key.segment_index,
        state.parameters.len(),
        state.borrow.mutable_parameter_count,
        state.operations.len(),
        state.transitions.len()
    );

    for operation in graph.operations.span_or_empty(state.operations) {
        label.push('\n');
        label.push_str("  ");
        label.push_str(&operation_label(graph, operation));
    }

    for transition in graph.transitions.span_or_empty(state.transitions) {
        label.push('\n');
        label.push_str("  ");
        label.push_str(&transition_label(graph, transition));
    }

    label
}

fn operation_label(graph: &StateGraph, operation: &Operation) -> String {
    match &operation.kind {
        OperationKind::Assignment => {
            let OperationExpressionRefs::Assignment { target, value } = operation.expressions
            else {
                return format!("#{} assign", operation.statement_index);
            };
            format!(
                "#{} {} = {}",
                operation.statement_index,
                expression_label(graph, target),
                expression_label(graph, value)
            )
        }
        OperationKind::Call {
            has_receiver,
            receiver,
            target,
            ..
        } => {
            let arguments = match operation.expressions {
                OperationExpressionRefs::Call { arguments } => graph
                    .expressions
                    .expression_handles(arguments)
                    .iter()
                    .map(|argument| expression_label(graph, *argument))
                    .collect::<Vec<_>>()
                    .join(", "),
                _ => String::new(),
            };
            if *has_receiver {
                format!(
                    "#{} call {}.{}({})",
                    operation.statement_index,
                    receiver.as_str(),
                    target.as_str(),
                    arguments
                )
            } else {
                format!(
                    "#{} call {}({})",
                    operation.statement_index,
                    target.as_str(),
                    arguments
                )
            }
        }
        OperationKind::ConstantIntegerAssignment => {
            format!("#{} const-int assign", operation.statement_index)
        }
        OperationKind::Expression => match operation.expressions {
            OperationExpressionRefs::Expression(expression) => format!(
                "#{} expr {}",
                operation.statement_index,
                expression_label(graph, expression)
            ),
            _ => format!("#{} expr", operation.statement_index),
        },
        OperationKind::LocalData => format!("#{} local data", operation.statement_index),
        OperationKind::StaticAssignment => {
            format!("#{} static assign", operation.statement_index)
        }
    }
}

fn transition_label(graph: &StateGraph, transition: &TransitionEdge) -> String {
    let guard = expression_label_option(graph, transition.expressions.guard)
        .map(|guard| format!(" if {guard}"))
        .unwrap_or_default();
    format!(
        "#{} transition{} -> {}",
        transition.statement_index,
        guard,
        transition_target_label(&transition.target)
    )
}

fn transition_target_label(target: &PlannedTransitionTarget) -> String {
    match target {
        PlannedTransitionTarget::None => "none".to_owned(),
        PlannedTransitionTarget::State { name, .. } => name.to_string(),
        PlannedTransitionTarget::Nested {
            receiver, state, ..
        } => format!("{}.{}", receiver.as_str(), state.as_str()),
        PlannedTransitionTarget::SelfTarget => "self".to_owned(),
        PlannedTransitionTarget::Terminal => "terminal".to_owned(),
    }
}

fn append_transition_edges(
    diagram: &mut PhaseDiagramBuilder,
    graph: &StateGraph,
    state_nodes: &[(StateKey, String)],
    source_id: &str,
    transition: &TransitionEdge,
) {
    if let Some(target_id) = transition_target_id(graph, state_nodes, &transition.target) {
        diagram.edge(source_id, target_id, "transition_target");
    }

    if let Some(target_id) = transition_target_id(graph, state_nodes, &transition.continuation) {
        diagram.edge(source_id, target_id, "transition_continuation");
    }
}

fn operation_call_target_id<'nodes>(
    graph: &StateGraph,
    state_nodes: &'nodes [(StateKey, String)],
    operation: &Operation,
) -> Option<&'nodes str> {
    let OperationKind::Call {
        receiver_symbol,
        target_symbol,
        has_receiver,
        ..
    } = operation.kind
    else {
        return None;
    };

    if has_receiver {
        return graph
            .state_key_by_symbols(receiver_symbol, target_symbol)
            .and_then(|key| state_id_for_key(state_nodes, key));
    }

    state_id_for_state_symbol(graph, state_nodes, target_symbol)
}

fn operation_external_call_scope_id<'nodes>(
    graph: &StateGraph,
    machine_nodes: &'nodes [(SymbolHandle, String, String)],
    source_machine: &MachineGraph,
    operation: &Operation,
) -> Option<&'nodes str> {
    let OperationKind::Call {
        receiver_symbol,
        target_symbol,
        has_receiver,
        receiver,
        target,
    } = &operation.kind
    else {
        return None;
    };

    if *has_receiver {
        return receiver_machine_scope_id(
            graph,
            machine_nodes,
            source_machine,
            *receiver_symbol,
            receiver.as_str(),
        );
    }

    if target_symbol.is_valid() {
        if let Some(state) = graph
            .states
            .iter()
            .map(|(_, state)| state)
            .find(|state| state.key.state == *target_symbol)
        {
            return machine_id_for_symbol(machine_nodes, state.key.machine);
        }
    }

    unique_machine_id_for_state_name(graph, machine_nodes, target.as_str())
}

fn receiver_machine_scope_id<'nodes>(
    graph: &StateGraph,
    machine_nodes: &'nodes [(SymbolHandle, String, String)],
    source_machine: &MachineGraph,
    receiver_symbol: SymbolHandle,
    receiver_name: &str,
) -> Option<&'nodes str> {
    if receiver_symbol == source_machine.symbol
        || names_match(source_machine.name.as_str(), receiver_name)
    {
        return machine_id_for_symbol(machine_nodes, source_machine.symbol);
    }

    for contained in graph.machine_contains(source_machine) {
        if contained.symbol == receiver_symbol
            || names_match(contained.name.as_str(), receiver_name)
        {
            return machine_id_for_symbol(machine_nodes, contained.type_symbol)
                .or_else(|| machine_id_for_name(machine_nodes, contained.type_name.as_str()));
        }
    }

    for owned_data in graph.machine_owned_data(source_machine) {
        if owned_data.symbol == receiver_symbol
            || names_match(owned_data.name.as_str(), receiver_name)
        {
            return machine_id_for_name(machine_nodes, owned_data.name.as_str());
        }
    }

    machine_id_for_name(machine_nodes, receiver_name)
}

fn unique_machine_id_for_state_name<'nodes>(
    graph: &StateGraph,
    machine_nodes: &'nodes [(SymbolHandle, String, String)],
    state_name: &str,
) -> Option<&'nodes str> {
    let mut matches = graph
        .states
        .iter()
        .map(|(_, state)| state)
        .filter(|state| names_match(state.name.as_str(), state_name))
        .filter_map(|state| machine_id_for_symbol(machine_nodes, state.key.machine));
    let first = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(first)
}

fn machine_id_for_symbol(
    machine_nodes: &[(SymbolHandle, String, String)],
    symbol: SymbolHandle,
) -> Option<&str> {
    machine_nodes
        .iter()
        .find(|(machine_symbol, _, _)| *machine_symbol == symbol)
        .map(|(_, _, id)| id.as_str())
}

fn machine_id_for_name<'nodes>(
    machine_nodes: &'nodes [(SymbolHandle, String, String)],
    name: &str,
) -> Option<&'nodes str> {
    machine_nodes
        .iter()
        .find(|(_, machine_name, _)| names_match(machine_name, name))
        .map(|(_, _, id)| id.as_str())
}

fn names_match(left: &str, right: &str) -> bool {
    normalized_name(left) == normalized_name(right)
}

fn normalized_name(value: &str) -> String {
    value
        .chars()
        .filter(|ch| *ch != '_' && *ch != ':' && *ch != '.')
        .flat_map(char::to_lowercase)
        .collect()
}

fn transition_target_id<'nodes>(
    graph: &StateGraph,
    state_nodes: &'nodes [(StateKey, String)],
    target: &PlannedTransitionTarget,
) -> Option<&'nodes str> {
    match target {
        PlannedTransitionTarget::State { key, .. } => state_id_for_key(state_nodes, *key),
        PlannedTransitionTarget::Nested { state_symbol, .. } => {
            state_id_for_state_symbol(graph, state_nodes, *state_symbol)
        }
        PlannedTransitionTarget::None
        | PlannedTransitionTarget::SelfTarget
        | PlannedTransitionTarget::Terminal => None,
    }
}

fn state_id_for_key(state_nodes: &[(StateKey, String)], key: StateKey) -> Option<&str> {
    state_nodes
        .iter()
        .find(|(candidate, _)| *candidate == key)
        .map(|(_, id)| id.as_str())
}

fn state_id_for_state_symbol<'nodes>(
    graph: &StateGraph,
    state_nodes: &'nodes [(StateKey, String)],
    state_symbol: SymbolHandle,
) -> Option<&'nodes str> {
    let mut matches = state_nodes
        .iter()
        .filter(|(key, _)| key.state == state_symbol && graph.state_by_key(*key).is_some())
        .map(|(_, id)| id.as_str());
    let first = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(first)
}

fn expression_label_option(graph: &StateGraph, expression: ExpressionHandle) -> Option<String> {
    if expression.is_valid() {
        Some(graph.expressions.display_name(expression))
    } else {
        None
    }
}

fn expression_label(graph: &StateGraph, expression: ExpressionHandle) -> String {
    expression_label_option(graph, expression).unwrap_or_else(|| "_".to_owned())
}
