use omega_checked_trees::{CheckedTrees, FlowInvalidationSource};
use omega_core::arena::{Arena, HandleSpan};
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

pub(crate) fn remap_state_ownership_summary(
    target: &mut StateGraph,
    source_segments: &Arena<omega_facts::PlaceSegment>,
    source_moves: &Arena<StateMoveEvent>,
    source_drops: &Arena<StateDropEvent>,
    ownership: &StateOwnershipSummary,
) -> StateOwnershipSummary {
    let moves = append_remapped_move_events(target, source_segments, source_moves, ownership.moves);
    let drops = append_remapped_drop_events(target, source_segments, source_drops, ownership.drops);

    StateOwnershipSummary { moves, drops }
}

fn append_remapped_move_events(
    target: &mut StateGraph,
    source_segments: &Arena<omega_facts::PlaceSegment>,
    source_moves: &Arena<StateMoveEvent>,
    moves: HandleSpan<StateMoveEvent>,
) -> HandleSpan<StateMoveEvent> {
    let mut remapped = HandleSpan::empty();

    for event in source_moves.span_or_empty(moves) {
        target.move_events.append_to_span(
            &mut remapped,
            StateMoveEvent {
                source: event.source,
                root: event.root,
                segments: target.ownership_segments.insert_many(
                    source_segments
                        .span_or_empty(event.segments)
                        .iter()
                        .copied(),
                ),
            },
        );
    }

    remapped
}

fn append_remapped_drop_events(
    target: &mut StateGraph,
    source_segments: &Arena<omega_facts::PlaceSegment>,
    source_drops: &Arena<StateDropEvent>,
    drops: HandleSpan<StateDropEvent>,
) -> HandleSpan<StateDropEvent> {
    let mut remapped = HandleSpan::empty();

    for event in source_drops.span_or_empty(drops) {
        target.drop_events.append_to_span(
            &mut remapped,
            StateDropEvent {
                source: event.source,
                root: event.root,
                segments: target.ownership_segments.insert_many(
                    source_segments
                        .span_or_empty(event.segments)
                        .iter()
                        .copied(),
                ),
            },
        );
    }

    remapped
}

fn remap_flow_ownership_event_source(source: FlowInvalidationSource) -> StateOwnershipEventSource {
    match source {
        FlowInvalidationSource::Statement { statement_index } => {
            StateOwnershipEventSource::Statement { statement_index }
        }
        FlowInvalidationSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => StateOwnershipEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        },
    }
}
