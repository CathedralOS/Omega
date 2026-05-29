use omega_checked_trees::{CheckedTrees, FlowOwnershipEventSource};
use omega_core::arena::HandleSpan;
use omega_state_graph::{
    StateDropEvent, StateGraph, StateKey, StateMoveEvent, StateOwnershipEventSource,
    StateOwnershipSummary,
};

pub(crate) fn state_ownership_summary(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    key: StateKey,
) -> StateOwnershipSummary {
    let Some(flow_state) = program
        .facts
        .flow
        .states
        .iter()
        .find(|(_, state)| state.machine_symbol == key.machine && state.state_symbol == key.state)
        .map(|(_, state)| state)
    else {
        return StateOwnershipSummary::default();
    };

    let mut moves = HandleSpan::empty();
    for event in program.facts.flow.moves.span_or_empty(flow_state.moves) {
        state_graph.move_events.append_to_span(
            &mut moves,
            StateMoveEvent {
                source: remap_flow_ownership_event_source(event.source),
                root: event.root,
                segments: state_graph.ownership_segments.insert_many(
                    program
                        .facts
                        .flow
                        .ownership_segments
                        .span_or_empty(event.segments)
                        .iter()
                        .copied(),
                ),
            },
        );
    }

    let mut drops = HandleSpan::empty();
    for event in program.facts.flow.drops.span_or_empty(flow_state.drops) {
        state_graph.drop_events.append_to_span(
            &mut drops,
            StateDropEvent {
                source: remap_flow_ownership_event_source(event.source),
                root: event.root,
                segments: state_graph.ownership_segments.insert_many(
                    program
                        .facts
                        .flow
                        .ownership_segments
                        .span_or_empty(event.segments)
                        .iter()
                        .copied(),
                ),
            },
        );
    }

    StateOwnershipSummary { moves, drops }
}

fn remap_flow_ownership_event_source(
    source: FlowOwnershipEventSource,
) -> StateOwnershipEventSource {
    match source {
        FlowOwnershipEventSource::Statement { statement_index } => {
            StateOwnershipEventSource::Statement { statement_index }
        }
        FlowOwnershipEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => StateOwnershipEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        },
        FlowOwnershipEventSource::StateExit => StateOwnershipEventSource::StateExit,
    }
}
