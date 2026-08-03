use omega_state_graph::{
    PlannedTransitionTarget, StateGraph, StateNode, TransitionEdge, TransitionExpressionRefs,
};
use psi_arena::HandleSpan;
use psi_checked_trees::CheckedTrees;
use psi_diagnostics::Diagnostic;

use crate::borrows::state_borrow_summary;
use crate::boundaries::state_boundary_summary;
use crate::contracts::state_contract_summary;
use crate::machine_metadata::{state_operational_summary, state_service_reach};
use crate::ownership::state_ownership_summary;
use crate::segments::{SegmentTransition, StateSegment, segment_has_unconditional_transition};
use crate::transitions::plan_transition;
use crate::values::state_value_summary;

pub(crate) fn append_machine_states(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    segments: &[StateSegment],
    segment_transitions: &psi_arena::Arena<SegmentTransition>,
) -> Result<HandleSpan<StateNode>, Diagnostic> {
    let mut states = HandleSpan::empty();

    for (index, segment) in segments.iter().enumerate() {
        let transitions = append_segment_transitions(
            state_graph,
            program,
            segment,
            segments,
            segment_transitions,
        )?;
        let contracts = state_contract_summary(state_graph, program, segment, segment_transitions);
        let values = state_value_summary(state_graph, program, segment.key);
        let boundaries = state_boundary_summary(state_graph, program, segment.key);
        let borrow = state_borrow_summary(state_graph, program, segment.key);
        let ownership = state_ownership_summary(state_graph, program, segment.key);
        state_graph.states.append_to_span(
            &mut states,
            StateNode {
                key: segment.key,
                name: segment.name.clone(),
                index,
                service_reach: state_service_reach(program, segment.key.state),
                operational: state_operational_summary(program, segment.key.state),
                parameters: segment.parameters,
                contracts,
                values,
                boundaries,
                borrow,
                ownership,
                operations: segment.operations,
                transitions,
            },
        );
    }

    Ok(states)
}

fn append_segment_transitions(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    segment: &StateSegment,
    segments: &[StateSegment],
    segment_transitions: &psi_arena::Arena<SegmentTransition>,
) -> Result<HandleSpan<TransitionEdge>, Diagnostic> {
    let mut transitions = HandleSpan::empty();

    for transition in segment_transitions.span_or_empty(segment.transitions) {
        let transition = plan_transition(segment.key, segments, transition, program, state_graph)?;
        state_graph
            .transitions
            .append_to_span(&mut transitions, transition);
    }

    if segment.next_segment_key.is_valid()
        && !segment_has_unconditional_transition(segment, segment_transitions)
    {
        let next_segment_key = segment.next_segment_key;
        let (next_index, next_segment) = segments
            .iter()
            .enumerate()
            .find(|(_, segment)| segment.key == next_segment_key)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "internal state-graph segment #{} was not indexed",
                    next_segment_key.segment_index
                ))
            })?;

        state_graph.transitions.append_to_span(
            &mut transitions,
            TransitionEdge {
                statement_index: 0,
                target: PlannedTransitionTarget::State {
                    index: next_index,
                    key: next_segment.key,
                    name: next_segment.name.clone(),
                },
                continuation: PlannedTransitionTarget::None,
                expressions: TransitionExpressionRefs::default(),
            },
        );
    }

    Ok(transitions)
}
