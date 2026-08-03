use omega_control_flow::{
    PlannedTransitionTarget, StateKey, TransitionExpressionRefs, TransitionFlow,
};
use omega_state_graph::{StateGraph, TransitionEdge};
use psi_arena::Arena;

use crate::arena_remap::remap_arena;

pub(crate) fn remap_transitions(state_graph: &StateGraph) -> Arena<TransitionFlow> {
    remap_arena(&state_graph.transitions, remap_transition_owned)
}

pub(crate) fn remap_transition_owned(transition: TransitionEdge) -> TransitionFlow {
    TransitionFlow {
        statement_index: transition.statement_index,
        target: remap_transition_target_owned(transition.target),
        continuation: remap_transition_target_owned(transition.continuation),
        expressions: remap_transition_expression_refs(transition.expressions),
    }
}

pub(crate) fn remap_state_key(key: omega_state_graph::StateKey) -> StateKey {
    StateKey {
        machine: key.machine,
        state: key.state,
        segment_index: key.segment_index,
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

fn remap_transition_expression_refs(
    expressions: omega_state_graph::TransitionExpressionRefs,
) -> TransitionExpressionRefs {
    TransitionExpressionRefs {
        target_arguments: expressions.target_arguments,
        target_value: expressions.target_value,
        continuation_arguments: expressions.continuation_arguments,
        continuation_value: expressions.continuation_value,
        guard: expressions.guard,
    }
}

#[cfg(test)]
mod tests;
