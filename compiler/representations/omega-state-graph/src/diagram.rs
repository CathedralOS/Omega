use crate::{
    MachineGraph, Operation, OperationExpressionRefs, OperationKind, PlannedTransitionTarget,
    StateGraph, StateKey, StateNode, TransitionEdge,
};
use omega_core::diagnostics::{PhaseDiagram, PhaseDiagramBuilder};
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::expression::ExpressionHandle;

impl PhaseDiagram for StateGraph {
    fn phase_html(&self) -> String {
        let mut diagram = PhaseDiagramBuilder::new("state_graph");
        let mut state_nodes = Vec::new();

        for (machine_index, machine) in self.machines.iter() {
            let mut seen_state_keys = Vec::new();
            for state in self.states.span_or_empty(machine.states) {
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
                    state_label(self, machine, state),
                    "state_block",
                    machine_index.arena_index() as usize,
                );
                state_nodes.push((state.key, state_id));
            }
        }

        for (_, machine) in self.machines.iter() {
            for state in self.states.span_or_empty(machine.states) {
                let Some(source_id) = state_id_for_key(&state_nodes, state.key) else {
                    continue;
                };

                for operation in self.operations.span_or_empty(state.operations) {
                    if let Some(target_id) = operation_call_target_id(self, &state_nodes, operation)
                    {
                        diagram.edge(source_id, target_id, "call");
                    }
                }

                for transition in self.transitions.span_or_empty(state.transitions) {
                    append_transition_edges(
                        &mut diagram,
                        self,
                        &state_nodes,
                        source_id,
                        transition,
                    );
                }
            }
        }

        diagram.finish()
    }
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
