use super::*;
use omega_core::symbols::SymbolHandle;

#[test]
fn remap_ownership_summary_preserves_move_and_drop_event_handles() {
    let target_symbol = SymbolHandle::from_arena_index(1);

    let move_event = omega_state_graph::StateMoveEvent {
        source: omega_state_graph::StateOwnershipEventSource::Call {
            statement_index: 2,
            call_ordinal: 3,
            target_symbol,
        },
        root: Default::default(),
        segments: Default::default(),
    };
    let drop_event = omega_state_graph::StateDropEvent {
        source: omega_state_graph::StateOwnershipEventSource::StateExit,
        root: Default::default(),
        segments: Default::default(),
    };
    let mut moves = Arena::new();
    let mut drops = Arena::new();
    let mut move_span = omega_core::arena::HandleSpan::empty();
    let mut drop_span = omega_core::arena::HandleSpan::empty();
    moves.append_to_span(&mut move_span, move_event);
    drops.append_to_span(&mut drop_span, drop_event);

    let summary = remap_ownership_summary(&omega_state_graph::StateOwnershipSummary {
        moves: move_span,
        drops: drop_span,
    });

    assert_eq!(summary.moves.count(), 1);
    assert_eq!(summary.drops.count(), 1);
    assert_eq!(
        summary.moves.start().arena_index(),
        move_span.start().arena_index()
    );
    assert_eq!(
        summary.drops.start().arena_index(),
        drop_span.start().arena_index()
    );
}

#[test]
fn remap_owned_move_event_preserves_call_source_and_place() {
    let target_symbol = SymbolHandle::from_arena_index(1);
    let event = omega_state_graph::StateMoveEvent {
        source: omega_state_graph::StateOwnershipEventSource::Call {
            statement_index: 2,
            call_ordinal: 3,
            target_symbol,
        },
        root: Default::default(),
        segments: Default::default(),
    };

    let remapped = remap_move_event_owned(event);

    assert_eq!(remapped.root, Default::default());
    assert_eq!(
        remapped.source,
        StateOwnershipEventSource::Call {
            statement_index: 2,
            call_ordinal: 3,
            target_symbol,
        }
    );
}
