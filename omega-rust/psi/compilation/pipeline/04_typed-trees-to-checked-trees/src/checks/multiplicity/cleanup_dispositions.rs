//! Exact remaining custody at a selected cleanup occurrence.
//!
//! This is a projection of the permission producer's occurrence order, not a
//! second control-flow analysis. The producer forks this state alongside its
//! ordinary ownership state for each outgoing edge.
use arena::Arena;
use checked_trees::{
    ContextualCleanupDisposition, ContextualCleanupFacts, FlowPermissionEventFact,
};
use facts::{Place, PlaceRoot, PlaceSegment};
use language_semantics::{PermissionAccess, PermissionEventKind};

#[derive(Clone, Default)]
pub(super) struct RemainingCustody {
    transferred: Vec<(PlaceRoot, Vec<PlaceSegment>)>,
}

impl RemainingCustody {
    pub(super) fn observe(
        &mut self,
        events: &[FlowPermissionEventFact],
        segments: &Arena<PlaceSegment>,
        output: &mut ContextualCleanupFacts,
    ) {
        for event in events {
            if event.access != PermissionAccess::Owned {
                continue;
            }
            let path = segments.span_or_empty(event.segments);
            if event.kind == PermissionEventKind::AffineDrop {
                let exclusions = self
                    .transferred
                    .iter()
                    .filter(|(root, _)| *root == event.root)
                    .map(|(root, path)| Place {
                        root: *root,
                        segments: output.segments.insert_many(path.iter().copied()),
                    })
                    .collect::<Vec<_>>();
                output.dispositions.append(ContextualCleanupDisposition {
                    machine: event.machine_symbol,
                    state: event.state_symbol,
                    source: event.source,
                    root: event.root,
                    path: event.segments,
                    exclusions: output.exclusions.insert_many(exclusions),
                });
            }
            self.apply(event, path);
        }
    }

    /// Query the same path retirement law for a projected replacement. The
    /// caller supplies only its current state's already-emitted occurrences.
    pub(super) fn removed_after<'a>(
        events: impl Iterator<Item = &'a FlowPermissionEventFact>,
        segments: &Arena<PlaceSegment>,
        root: PlaceRoot,
        path: &[PlaceSegment],
    ) -> bool {
        let mut remaining = Self::default();
        for event in events.filter(|event| event.access == PermissionAccess::Owned) {
            let event_path = segments.span_or_empty(event.segments);
            remaining.apply(event, event_path);
        }
        remaining
            .transferred
            .iter()
            .any(|(candidate, removed)| *candidate == root && path.starts_with(removed))
    }

    fn apply(&mut self, event: &FlowPermissionEventFact, path: &[PlaceSegment]) {
        match event.kind {
            PermissionEventKind::Establish => {
                // A strict child cannot revive its consumed containing owner.
                self.transferred
                    .retain(|(root, removed)| *root != event.root || !removed.starts_with(path));
            }
            PermissionEventKind::Transfer
            | PermissionEventKind::Consume
            | PermissionEventKind::AffineDrop => self.remove(event.root, path),
        }
    }

    fn remove(&mut self, root: PlaceRoot, path: &[PlaceSegment]) {
        if self
            .transferred
            .iter()
            .any(|(prior_root, prior)| *prior_root == root && path.starts_with(prior))
        {
            return;
        }
        self.transferred
            .retain(|(prior_root, prior)| *prior_root != root || !prior.starts_with(path));
        self.transferred.push((root, path.to_vec()));
    }
}
