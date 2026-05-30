use super::*;
use omega_core::symbols::SymbolHandle;

#[test]
fn remaps_ownership_summary_from_source_roots_into_target_roots() {
    let mut target = StateGraph::default();
    let mut segments = Arena::new();
    let mut moves = Arena::new();
    let mut drops = Arena::new();

    let mut segment_span = HandleSpan::empty();
    let mut move_span = HandleSpan::empty();
    let mut drop_span = HandleSpan::empty();

    segments.append_to_span(
        &mut segment_span,
        omega_facts::PlaceSegment::Field {
            symbol: SymbolHandle::from_arena_index(1),
        },
    );
    moves.append_to_span(
        &mut move_span,
        StateMoveEvent {
            source: omega_state_graph::StateOwnershipEventSource::Call {
                statement_index: 2,
                call_ordinal: 3,
                target_symbol: SymbolHandle::from_arena_index(4),
            },
            root: omega_facts::PlaceRoot::Symbol(SymbolHandle::from_arena_index(5)),
            segments: segment_span,
        },
    );
    drops.append_to_span(
        &mut drop_span,
        StateDropEvent {
            source: omega_state_graph::StateOwnershipEventSource::StateExit,
            root: omega_facts::PlaceRoot::Symbol(SymbolHandle::from_arena_index(6)),
            segments: segment_span,
        },
    );

    let remapped = remap_state_ownership_summary(
        &mut target,
        &SourceOwnershipArenas {
            segments: &segments,
            moves: &moves,
            drops: &drops,
        },
        &StateOwnershipSummary {
            moves: move_span,
            drops: drop_span,
        },
    );

    assert_eq!(remapped.moves.count(), 1);
    assert_eq!(remapped.drops.count(), 1);
    assert_eq!(target.semantics.ownership.moves.len(), 1);
    assert_eq!(target.semantics.ownership.drops.len(), 1);
    assert_eq!(target.semantics.ownership.segments.len(), 2);

    let move_event = target
        .semantics
        .ownership
        .moves
        .span_or_empty(remapped.moves)
        .first()
        .unwrap();
    assert_eq!(move_event.segments.count(), 1);
    assert_eq!(
        move_event.root,
        omega_facts::PlaceRoot::Symbol(SymbolHandle::from_arena_index(5))
    );
}
