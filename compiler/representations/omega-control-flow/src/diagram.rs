use crate::{
    ControlFlowPlan, MachineFlow, Operation, OperationExpressionRefs, OperationKind,
    PlannedTransitionTarget, StateFlow, StateKey, TransitionFlow,
};
use omega_core::diagnostics::{PhaseDiagram, PhaseDiagramBuilder};
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::expression::ExpressionHandle;

impl PhaseDiagram for ControlFlowPlan {
    fn phase_html(&self) -> String {
        let mut diagram = PhaseDiagramBuilder::new("control_flow");
        let root = diagram.node("root", "ControlFlowPlan", "root", 0);
        let mut state_nodes = Vec::new();

        for (machine_index, machine) in self.machines.iter() {
            let machine_id = diagram.node(
                format!("machine_{}", machine_index.arena_index()),
                machine_label(self, machine),
                "machine",
                1,
            );
            diagram.containment_edge(&root, &machine_id);

            let mut previous_state_id: Option<String> = None;
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
                    2,
                );
                diagram.containment_edge(&machine_id, &state_id);
                if let Some(previous_id) = previous_state_id.as_deref() {
                    diagram.sequence_edge(previous_id, &state_id);
                }
                previous_state_id = Some(state_id.clone());
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

fn machine_label(plan: &ControlFlowPlan, machine: &MachineFlow) -> String {
    format!(
        "machine {}\nstates: {}\ncontains: {}\nowns: {}",
        machine.name.as_str(),
        machine.states.len(),
        machine.contains.len(),
        plan.machine_owned_data(machine).len()
    )
}

fn state_label(plan: &ControlFlowPlan, machine: &MachineFlow, state: &StateFlow) -> String {
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

    for operation in plan.operations.span_or_empty(state.operations) {
        label.push('\n');
        label.push_str("  ");
        label.push_str(&operation_label(plan, operation));
    }

    for transition in plan.transitions.span_or_empty(state.transitions) {
        label.push('\n');
        label.push_str("  ");
        label.push_str(&transition_label(plan, transition));
    }

    label
}

fn operation_label(plan: &ControlFlowPlan, operation: &Operation) -> String {
    match &operation.kind {
        OperationKind::Assignment => {
            let OperationExpressionRefs::Assignment { target, value } = operation.expressions
            else {
                return format!("#{} assign", operation.statement_index);
            };
            format!(
                "#{} {} = {}",
                operation.statement_index,
                expression_label(plan, target),
                expression_label(plan, value)
            )
        }
        OperationKind::Call {
            has_receiver,
            receiver,
            target,
            ..
        } => {
            let arguments = match operation.expressions {
                OperationExpressionRefs::Call { arguments } => plan
                    .expressions
                    .expression_handles(arguments)
                    .iter()
                    .map(|argument| expression_label(plan, *argument))
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
                expression_label(plan, expression)
            ),
            _ => format!("#{} expr", operation.statement_index),
        },
        OperationKind::LocalData => format!("#{} local data", operation.statement_index),
        OperationKind::StaticAssignment => {
            format!("#{} static assign", operation.statement_index)
        }
    }
}

fn transition_label(plan: &ControlFlowPlan, transition: &TransitionFlow) -> String {
    let guard = expression_label_option(plan, transition.expressions.guard)
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
    plan: &ControlFlowPlan,
    state_nodes: &[(StateKey, String)],
    source_id: &str,
    transition: &TransitionFlow,
) {
    if let Some(target_id) = transition_target_id(plan, state_nodes, &transition.target) {
        diagram.edge(source_id, target_id, "transition_target");
    }

    if let Some(target_id) = transition_target_id(plan, state_nodes, &transition.continuation) {
        diagram.edge(source_id, target_id, "transition_continuation");
    }
}

fn operation_call_target_id<'nodes>(
    plan: &ControlFlowPlan,
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
        return plan
            .state_key_by_symbols(receiver_symbol, target_symbol)
            .and_then(|key| state_id_for_key(state_nodes, key));
    }

    state_id_for_state_symbol(plan, state_nodes, target_symbol)
}

fn transition_target_id<'nodes>(
    plan: &ControlFlowPlan,
    state_nodes: &'nodes [(StateKey, String)],
    target: &PlannedTransitionTarget,
) -> Option<&'nodes str> {
    match target {
        PlannedTransitionTarget::State { key, .. } => state_id_for_key(state_nodes, *key),
        PlannedTransitionTarget::Nested { state_symbol, .. } => {
            state_id_for_state_symbol(plan, state_nodes, *state_symbol)
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
    plan: &ControlFlowPlan,
    state_nodes: &'nodes [(StateKey, String)],
    state_symbol: SymbolHandle,
) -> Option<&'nodes str> {
    let mut matches = state_nodes
        .iter()
        .filter(|(key, _)| key.state == state_symbol && plan.state_by_key(*key).is_some())
        .map(|(_, id)| id.as_str());
    let first = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(first)
}

fn expression_label_option(plan: &ControlFlowPlan, expression: ExpressionHandle) -> Option<String> {
    if expression.is_valid() {
        Some(plan.expressions.display_name(expression))
    } else {
        None
    }
}

fn expression_label(plan: &ControlFlowPlan, expression: ExpressionHandle) -> String {
    expression_label_option(plan, expression).unwrap_or_else(|| "_".to_owned())
}
