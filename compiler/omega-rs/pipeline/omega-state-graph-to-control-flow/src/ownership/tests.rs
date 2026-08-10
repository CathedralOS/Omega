use super::*;

#[test]
fn remap_ownership_summary_preserves_permission_handles() {
    let mut permissions = Arena::new();
    let mut permission_span = psi_arena::HandleSpan::empty();
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
        permissions: permission_span,
    });

    assert_eq!(summary.permissions.count(), 1);
    assert_eq!(
        summary.permissions.start().arena_index(),
        permission_span.start().arena_index()
    );
}
