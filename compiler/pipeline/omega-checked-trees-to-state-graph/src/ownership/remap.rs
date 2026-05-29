use omega_core::arena::{Arena, HandleSpan};
use omega_state_graph::{StateDropEvent, StateGraph, StateMoveEvent, StateOwnershipSummary};

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
