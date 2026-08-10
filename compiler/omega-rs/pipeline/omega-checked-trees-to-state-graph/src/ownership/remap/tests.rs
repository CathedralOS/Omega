use super::*;
use psi_symbols::SymbolHandle;

#[test]
fn remaps_ownership_summary_from_source_roots_into_target_roots() {
    let mut target = StateGraph::default();
    let mut segments = Arena::new();
    let mut permissions = Arena::new();

    let mut segment_span = HandleSpan::empty();
    let mut permission_span = HandleSpan::empty();

    segments.append_to_span(
        &mut segment_span,
        psi_facts::PlaceSegment::Field {
            symbol: SymbolHandle::from_arena_index(1),
        },
    );
    permissions.append_to_span(
        &mut permission_span,
        StatePermissionEvent {
            source: psi_language_semantics::PermissionEventSource::Statement { statement_index: 7 },
            kind: psi_language_semantics::PermissionEventKind::Consume,
            multiplicity: psi_language_semantics::Multiplicity::Linear,
            access: psi_language_semantics::PermissionAccess::Owned,
            claim_identity: psi_language_semantics::PermissionClaimIdentity::Unknown,
            provenance: psi_language_semantics::PermissionProvenance::Unknown,
            root: psi_facts::PlaceRoot::Symbol(SymbolHandle::from_arena_index(7)),
            segments: segment_span,
            obligation_live: true,
        },
    );

    let remapped = remap_state_ownership_summary(
        &mut target,
        &SourceOwnershipArenas {
            segments: &segments,
            permissions: &permissions,
        },
        &StateOwnershipSummary {
            permissions: permission_span,
        },
    );

    assert_eq!(remapped.permissions.count(), 1);
    assert_eq!(target.semantics.ownership.permissions.len(), 1);
    assert_eq!(target.semantics.ownership.segments.len(), 1);
    let permission = target
        .semantics
        .ownership
        .permissions
        .span_or_empty(remapped.permissions)
        .first()
        .unwrap();
    assert_eq!(
        permission.kind,
        psi_language_semantics::PermissionEventKind::Consume
    );
    assert_eq!(permission.segments.count(), 1);
}
