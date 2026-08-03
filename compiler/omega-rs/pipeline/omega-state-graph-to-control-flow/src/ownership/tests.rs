use super::*;
use psi_symbols::SymbolHandle;

#[test]
fn remap_ownership_summary_preserves_all_event_handles() {
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
    let mut permissions = Arena::new();
    let mut move_span = psi_arena::HandleSpan::empty();
    let mut drop_span = psi_arena::HandleSpan::empty();
    let mut permission_span = psi_arena::HandleSpan::empty();
    moves.append_to_span(&mut move_span, move_event);
    drops.append_to_span(&mut drop_span, drop_event);
    permissions.append_to_span(
        &mut permission_span,
        omega_state_graph::StatePermissionEvent {
            source: psi_language_semantics::PermissionEventSource::StateEntry,
            kind: psi_language_semantics::PermissionEventKind::Establish,
            multiplicity: psi_language_semantics::Multiplicity::Linear,
            access: psi_language_semantics::PermissionAccess::Owned,
            claim_identity: psi_language_semantics::PermissionClaimIdentity::Unknown,
            provenance: psi_language_semantics::PermissionProvenance::Unknown,
            root: Default::default(),
            segments: Default::default(),
            obligation_live: true,
        },
    );

    let summary = remap_ownership_summary(&omega_state_graph::StateOwnershipSummary {
        moves: move_span,
        drops: drop_span,
        permissions: permission_span,
    });

    assert_eq!(summary.moves.count(), 1);
    assert_eq!(summary.drops.count(), 1);
    assert_eq!(summary.permissions.count(), 1);
    assert_eq!(
        summary.moves.start().arena_index(),
        move_span.start().arena_index()
    );
    assert_eq!(
        summary.drops.start().arena_index(),
        drop_span.start().arena_index()
    );
    assert_eq!(
        summary.permissions.start().arena_index(),
        permission_span.start().arena_index()
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
