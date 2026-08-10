use omega_state_graph::{StateGraph, StateOwnershipSummary, StatePermissionEvent};
use psi_arena::{Arena, HandleSpan};

pub(crate) struct SourceOwnershipArenas<'a> {
    pub(crate) segments: &'a Arena<psi_facts::PlaceSegment>,
    pub(crate) permissions: &'a Arena<StatePermissionEvent>,
}

pub(crate) fn remap_state_ownership_summary(
    target: &mut StateGraph,
    source: &SourceOwnershipArenas<'_>,
    ownership: &StateOwnershipSummary,
) -> StateOwnershipSummary {
    let permissions = append_remapped_permission_events(target, source, ownership.permissions);

    StateOwnershipSummary { permissions }
}

fn append_remapped_permission_events(
    target: &mut StateGraph,
    source: &SourceOwnershipArenas<'_>,
    permissions: HandleSpan<StatePermissionEvent>,
) -> HandleSpan<StatePermissionEvent> {
    let mut remapped = HandleSpan::empty();
    for event in source.permissions.span_or_empty(permissions) {
        target.semantics.ownership.permissions.append_to_span(
            &mut remapped,
            StatePermissionEvent {
                source: event.source,
                kind: event.kind,
                multiplicity: event.multiplicity,
                access: event.access,
                claim_identity: event.claim_identity,
                provenance: event.provenance,
                root: event.root,
                segments: target.semantics.ownership.segments.insert_many(
                    source
                        .segments
                        .span_or_empty(event.segments)
                        .iter()
                        .copied(),
                ),
                obligation_live: event.obligation_live,
            },
        );
    }
    remapped
}

#[cfg(test)]
mod tests;
