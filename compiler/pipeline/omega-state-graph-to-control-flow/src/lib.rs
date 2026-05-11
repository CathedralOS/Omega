use omega_control_flow::{
    ContainedFlow, ControlFlowPlan, MachineFlow, Operation, OperationExpressionRefs,
    OperationKind, PlannedTransitionTarget, StateFlow, StateKey, StateParameterFlow,
    TransitionExpressionRefs, TransitionFlow,
};
use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_state_graph::{
    ContainedGraph, MachineGraph, StateGraph, StateNode, StateParameterNode, TransitionEdge,
};

pub fn build_control_flow_plan(state_graph: &StateGraph) -> Result<ControlFlowPlan, Diagnostic> {
    Ok(ControlFlowPlan {
        expressions: state_graph.expressions.clone(),
        machines: remap_machines(state_graph),
        states: remap_states(state_graph),
        operations: remap_operations(state_graph),
        transitions: remap_transitions(state_graph),
    })
}

fn remap_machines(state_graph: &StateGraph) -> Arena<MachineFlow> {
    let mut machines = Arena::default();

    for (_, machine) in state_graph.machines.iter() {
        machines.append(remap_machine(machine));
    }

    machines
}

fn remap_machine(machine: &MachineGraph) -> MachineFlow {
    MachineFlow {
        symbol: machine.symbol,
        name: machine.name.clone(),
        contains: machine.contains.iter().map(remap_contained).collect(),
        states: remap_state_span(machine.states),
    }
}

fn remap_contained(contained: &ContainedGraph) -> ContainedFlow {
    ContainedFlow {
        symbol: contained.symbol,
        name: contained.name.clone(),
        type_symbol: contained.type_symbol,
        type_name: contained.type_name.clone(),
    }
}

fn remap_states(state_graph: &StateGraph) -> Arena<StateFlow> {
    let mut states = Arena::default();

    for (_, state) in state_graph.states.iter() {
        states.append(remap_state(state));
    }

    states
}

fn remap_state(state: &StateNode) -> StateFlow {
    StateFlow {
        key: remap_state_key(state.key),
        name: state.name.clone(),
        index: state.index,
        parameters: state.parameters.iter().map(remap_parameter).collect(),
        operations: remap_operation_span(state.operations),
        transitions: remap_transition_span(state.transitions),
    }
}

fn remap_parameter(parameter: &StateParameterNode) -> StateParameterFlow {
    StateParameterFlow {
        symbol: parameter.symbol,
        name: parameter.name.clone(),
        is_mutable_reference: parameter.is_mutable_reference,
    }
}

fn remap_operations(state_graph: &StateGraph) -> Arena<Operation> {
    let mut operations = Arena::default();

    for (_, operation) in state_graph.operations.iter() {
        operations.append(remap_operation(operation));
    }

    operations
}

fn remap_transitions(state_graph: &StateGraph) -> Arena<TransitionFlow> {
    let mut transitions = Arena::default();

    for (_, transition) in state_graph.transitions.iter() {
        transitions.append(remap_transition(transition));
    }

    transitions
}

fn remap_operation(operation: &omega_state_graph::Operation) -> Operation {
    Operation {
        statement_index: operation.statement_index,
        kind: remap_operation_kind(&operation.kind),
        expressions: remap_operation_expression_refs(operation.expressions),
    }
}

fn remap_operation_kind(kind: &omega_state_graph::OperationKind) -> OperationKind {
    match kind {
        omega_state_graph::OperationKind::Assignment => OperationKind::Assignment,
        omega_state_graph::OperationKind::Call {
            receiver_symbol,
            target_symbol,
            receiver,
            target,
        } => OperationKind::Call {
            receiver_symbol: *receiver_symbol,
            target_symbol: *target_symbol,
            receiver: receiver.clone(),
            target: target.clone(),
        },
        omega_state_graph::OperationKind::ConstantIntegerAssignment => {
            OperationKind::ConstantIntegerAssignment
        }
        omega_state_graph::OperationKind::Expression => OperationKind::Expression,
        omega_state_graph::OperationKind::LocalData => OperationKind::LocalData,
        omega_state_graph::OperationKind::StaticAssignment => OperationKind::StaticAssignment,
    }
}

fn remap_operation_expression_refs(
    expressions: omega_state_graph::OperationExpressionRefs,
) -> OperationExpressionRefs {
    match expressions {
        omega_state_graph::OperationExpressionRefs::Assignment { target, value } => {
            OperationExpressionRefs::Assignment { target, value }
        }
        omega_state_graph::OperationExpressionRefs::Call { arguments } => {
            OperationExpressionRefs::Call {
                arguments: remap_expression_span(arguments),
            }
        }
        omega_state_graph::OperationExpressionRefs::Expression(expression) => {
            OperationExpressionRefs::Expression(expression)
        }
        omega_state_graph::OperationExpressionRefs::None => OperationExpressionRefs::None,
    }
}

fn remap_transition(transition: &TransitionEdge) -> TransitionFlow {
    TransitionFlow {
        target: remap_transition_target(&transition.target),
        continuation: transition.continuation.as_ref().map(remap_transition_target),
        guard: transition.guard.clone(),
        expressions: TransitionExpressionRefs {
            target_arguments: transition.expressions.target_arguments,
            target_value: transition.expressions.target_value,
            continuation_arguments: transition.expressions.continuation_arguments,
            continuation_value: transition.expressions.continuation_value,
            guard: transition.expressions.guard,
        },
    }
}

fn remap_state_span(states: HandleSpan<omega_state_graph::StateNode>) -> HandleSpan<StateFlow> {
    HandleSpan::from_parts(remap_state_handle(states.start()), states.count())
}

fn remap_state_handle(handle: Handle<omega_state_graph::StateNode>) -> Handle<StateFlow> {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

fn remap_operation_span(operations: HandleSpan<omega_state_graph::Operation>) -> HandleSpan<Operation> {
    HandleSpan::from_parts(remap_operation_handle(operations.start()), operations.count())
}

fn remap_operation_handle(
    handle: Handle<omega_state_graph::Operation>,
) -> Handle<Operation> {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

fn remap_transition_span(
    transitions: HandleSpan<omega_state_graph::TransitionEdge>,
) -> HandleSpan<TransitionFlow> {
    HandleSpan::from_parts(remap_transition_handle(transitions.start()), transitions.count())
}

fn remap_transition_handle(
    handle: Handle<omega_state_graph::TransitionEdge>,
) -> Handle<TransitionFlow> {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

fn remap_expression_span(
    expressions: HandleSpan<omega_typed_trees::expression::ExpressionHandle>,
) -> HandleSpan<omega_typed_trees::expression::ExpressionHandle> {
    HandleSpan::from_parts(expressions.start(), expressions.count())
}

fn remap_transition_target(target: &omega_state_graph::PlannedTransitionTarget) -> PlannedTransitionTarget {
    match target {
        omega_state_graph::PlannedTransitionTarget::State { index, key, name } => {
            PlannedTransitionTarget::State {
                index: *index,
                key: remap_state_key(*key),
                name: name.clone(),
            }
        }
        omega_state_graph::PlannedTransitionTarget::Nested {
            receiver_symbol,
            state_symbol,
            receiver,
            state,
        } => PlannedTransitionTarget::Nested {
            receiver_symbol: *receiver_symbol,
            state_symbol: *state_symbol,
            receiver: receiver.clone(),
            state: state.clone(),
        },
        omega_state_graph::PlannedTransitionTarget::SelfTarget => PlannedTransitionTarget::SelfTarget,
        omega_state_graph::PlannedTransitionTarget::Terminal => PlannedTransitionTarget::Terminal,
    }
}

fn remap_state_key(key: omega_state_graph::StateKey) -> StateKey {
    StateKey {
        machine: key.machine,
        state: key.state,
        segment_index: key.segment_index,
    }
}
