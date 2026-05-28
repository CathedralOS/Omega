use omega_control_flow::{
    PlannedTransitionTarget, StateKey, TransitionExpressionRefs, TransitionFlow,
};
use omega_core::arena::Arena;
use omega_state_graph::{StateGraph, TransitionEdge};

pub(crate) fn remap_transitions(state_graph: &StateGraph) -> Arena<TransitionFlow> {
    let mut transitions = Arena::with_capacity(state_graph.transitions.len());

    for (_, transition) in state_graph.transitions.iter() {
        transitions.append(remap_transition(transition));
    }

    transitions
}

pub(crate) fn remap_transition_owned(transition: TransitionEdge) -> TransitionFlow {
    TransitionFlow {
        statement_index: transition.statement_index,
        target: remap_transition_target_owned(transition.target),
        continuation: remap_transition_target_owned(transition.continuation),
        expressions: TransitionExpressionRefs {
            target_arguments: transition.expressions.target_arguments,
            target_value: transition.expressions.target_value,
            continuation_arguments: transition.expressions.continuation_arguments,
            continuation_value: transition.expressions.continuation_value,
            guard: transition.expressions.guard,
        },
    }
}

pub(crate) fn remap_state_key(key: omega_state_graph::StateKey) -> StateKey {
    StateKey {
        machine: key.machine,
        state: key.state,
        segment_index: key.segment_index,
    }
}

fn remap_transition(transition: &TransitionEdge) -> TransitionFlow {
    TransitionFlow {
        statement_index: transition.statement_index,
        target: remap_transition_target(&transition.target),
        continuation: remap_transition_target(&transition.continuation),
        expressions: TransitionExpressionRefs {
            target_arguments: transition.expressions.target_arguments,
            target_value: transition.expressions.target_value,
            continuation_arguments: transition.expressions.continuation_arguments,
            continuation_value: transition.expressions.continuation_value,
            guard: transition.expressions.guard,
        },
    }
}

fn remap_transition_target(
    target: &omega_state_graph::PlannedTransitionTarget,
) -> PlannedTransitionTarget {
    match target {
        omega_state_graph::PlannedTransitionTarget::None => PlannedTransitionTarget::None,
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
        omega_state_graph::PlannedTransitionTarget::SelfTarget => {
            PlannedTransitionTarget::SelfTarget
        }
        omega_state_graph::PlannedTransitionTarget::Terminal => PlannedTransitionTarget::Terminal,
    }
}

fn remap_transition_target_owned(
    target: omega_state_graph::PlannedTransitionTarget,
) -> PlannedTransitionTarget {
    match target {
        omega_state_graph::PlannedTransitionTarget::None => PlannedTransitionTarget::None,
        omega_state_graph::PlannedTransitionTarget::State { index, key, name } => {
            PlannedTransitionTarget::State {
                index,
                key: remap_state_key(key),
                name,
            }
        }
        omega_state_graph::PlannedTransitionTarget::Nested {
            receiver_symbol,
            state_symbol,
            receiver,
            state,
        } => PlannedTransitionTarget::Nested {
            receiver_symbol,
            state_symbol,
            receiver,
            state,
        },
        omega_state_graph::PlannedTransitionTarget::SelfTarget => {
            PlannedTransitionTarget::SelfTarget
        }
        omega_state_graph::PlannedTransitionTarget::Terminal => PlannedTransitionTarget::Terminal,
    }
}
