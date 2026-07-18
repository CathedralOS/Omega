use super::*;
use omega_core::symbols::SymbolHandle;

#[test]
fn remaps_ownership_summary_from_source_roots_into_target_roots() {
    let mut target = StateGraph::default();
    let mut segments = Arena::new();
    let mut moves = Arena::new();
    let mut drops = Arena::new();
    let mut permissions = Arena::new();

    let mut segment_span = HandleSpan::empty();
    let mut move_span = HandleSpan::empty();
    let mut drop_span = HandleSpan::empty();
    let mut permission_span = HandleSpan::empty();

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
    permissions.append_to_span(
        &mut permission_span,
        StatePermissionEvent {
            source: omega_core::semantics::PermissionEventSource::Statement {
                statement_index: 7,
            },
            kind: omega_core::semantics::PermissionEventKind::Consume,
            multiplicity: omega_core::semantics::Multiplicity::Linear,
            access: omega_core::semantics::PermissionAccess::Owned,
            provenance: omega_core::semantics::PermissionProvenance::Unknown,
            root: omega_facts::PlaceRoot::Symbol(SymbolHandle::from_arena_index(7)),
            segments: segment_span,
            obligation_live: true,
        },
    );

    let remapped = remap_state_ownership_summary(
        &mut target,
        &SourceOwnershipArenas {
            segments: &segments,
            moves: &moves,
            drops: &drops,
            permissions: &permissions,
        },
        &StateOwnershipSummary {
            moves: move_span,
            drops: drop_span,
            permissions: permission_span,
        },
    );

    assert_eq!(remapped.moves.count(), 1);
    assert_eq!(remapped.drops.count(), 1);
    assert_eq!(remapped.permissions.count(), 1);
    assert_eq!(target.semantics.ownership.moves.len(), 1);
    assert_eq!(target.semantics.ownership.drops.len(), 1);
    assert_eq!(target.semantics.ownership.permissions.len(), 1);
    assert_eq!(target.semantics.ownership.segments.len(), 3);

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
    let permission = target
        .semantics
        .ownership
        .permissions
        .span_or_empty(remapped.permissions)
        .first()
        .unwrap();
    assert_eq!(
        permission.kind,
        omega_core::semantics::PermissionEventKind::Consume
    );
}
