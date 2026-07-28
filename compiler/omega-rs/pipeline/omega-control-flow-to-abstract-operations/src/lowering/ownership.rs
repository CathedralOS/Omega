use omega_abstract_operations::{AbstractOwnershipSummary, AbstractPermissionEvent};
use omega_control_flow::ControlFlowPlan;

pub(super) fn build_abstract_ownership_summary(
    control_flow: &ControlFlowPlan,
) -> AbstractOwnershipSummary {
    let mut summary = AbstractOwnershipSummary::with_capacity(
        control_flow.semantics.ownership.segments.len(),
        control_flow.semantics.ownership.permissions.len(),
    );

    for (_, state) in control_flow.states.iter() {
        let permission_span = state.ownership.permissions;
        for (event_offset, event) in control_flow
            .semantics
            .ownership
            .permissions
            .span_or_empty(permission_span)
            .iter()
            .enumerate()
        {
            summary.permissions.insert(AbstractPermissionEvent {
                source_event_index: permission_span
                    .start()
                    .arena_index()
                    .checked_add(
                        u32::try_from(event_offset).expect("permission event offset overflow"),
                    )
                    .expect("permission event index overflow"),
                source_key: state.key,
                source: event.source,
                kind: event.kind,
                multiplicity: event.multiplicity,
                access: event.access,
                claim_identity: event.claim_identity,
                provenance: event.provenance,
                root: event.root,
                segments: summary.segments.insert_many(
                    control_flow
                        .semantics
                        .ownership
                        .segments
                        .span_or_empty(event.segments)
                        .iter()
                        .copied(),
                ),
                obligation_live: event.obligation_live,
            });
        }
    }

    summary
}

#[cfg(test)]
mod tests;
