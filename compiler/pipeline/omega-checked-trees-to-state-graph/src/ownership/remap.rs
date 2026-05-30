use omega_core::arena::{Arena, HandleSpan};
use omega_state_graph::{StateDropEvent, StateGraph, StateMoveEvent, StateOwnershipSummary};

pub(crate) struct SourceOwnershipArenas<'a> {
    pub(crate) segments: &'a Arena<omega_facts::PlaceSegment>,
    pub(crate) moves: &'a Arena<StateMoveEvent>,
    pub(crate) drops: &'a Arena<StateDropEvent>,
}

pub(crate) fn remap_state_ownership_summary(
    target: &mut StateGraph,
    source: &SourceOwnershipArenas<'_>,
    ownership: &StateOwnershipSummary,
) -> StateOwnershipSummary {
    let moves = append_remapped_move_events(target, source, ownership.moves);
    let drops = append_remapped_drop_events(target, source, ownership.drops);

    StateOwnershipSummary { moves, drops }
}

fn append_remapped_move_events(
    target: &mut StateGraph,
    source: &SourceOwnershipArenas<'_>,
    moves: HandleSpan<StateMoveEvent>,
) -> HandleSpan<StateMoveEvent> {
    let mut remapped = HandleSpan::empty();

    for event in source.moves.span_or_empty(moves) {
        target.semantics.ownership.moves.append_to_span(
            &mut remapped,
            StateMoveEvent {
                source: event.source,
                root: event.root,
                segments: target.semantics.ownership.segments.insert_many(
                    source
                        .segments
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
    source: &SourceOwnershipArenas<'_>,
    drops: HandleSpan<StateDropEvent>,
) -> HandleSpan<StateDropEvent> {
    let mut remapped = HandleSpan::empty();

    for event in source.drops.span_or_empty(drops) {
        target.semantics.ownership.drops.append_to_span(
            &mut remapped,
            StateDropEvent {
                source: event.source,
                root: event.root,
                segments: target.semantics.ownership.segments.insert_many(
                    source
                        .segments
                        .span_or_empty(event.segments)
                        .iter()
                        .copied(),
                ),
            },
        );
    }

    remapped
}

#[cfg(test)]
mod tests;
