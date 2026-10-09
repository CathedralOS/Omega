//! Exact remaining custody at a selected cleanup occurrence.
//!
//! This is a projection of the permission producer's occurrence order, not a
//! second control-flow analysis. The producer forks this state alongside its
//! ordinary ownership state for each outgoing edge.
use arena::Arena;
use checked_trees::{
    ContextualCleanupDisposition, ContextualCleanupFacts, FlowClaimJoinExit,
    FlowPermissionEventFact,
};
use facts::{Place, PlaceRoot, PlaceSegment};
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionEventKind, PermissionEventSource,
};

#[derive(Clone, Default)]
pub(crate) struct RemainingCustody {
    // Moved regions and conditional claim regions that were never established
    // both lie outside the owner's cleanup complement.
    excluded: Vec<(PlaceRoot, Vec<PlaceSegment>)>,
    constructing: Vec<(
        PlaceRoot,
        super::construction_frontier::ConstructionFrontier,
    )>,
}

/// A receiving write's storage extent, paired with its first claim event in
/// the current evaluation. Claim events name leaves, not the containing write.
pub(super) struct StorageEstablishment {
    pub(super) event_ordinal: usize,
    pub(super) destination: validation::CanonicalPlace,
}

impl RemainingCustody {
    /// Reserve a deferred owned local without inventing a valid whole value.
    pub(super) fn reserve(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        statement: &typed_trees::statement::StatementNode,
    ) {
        let Some(local) = statement.as_local_data() else {
            return;
        };
        if local.initial_value.is_valid()
            || program.type_multiplicity(local.type_reference)
                != language_semantics::Multiplicity::Affine
            || !validation::type_requires_establishment(program, local.type_reference)
        {
            return;
        }
        self.constructing.push((
            PlaceRoot::Symbol(local.symbol),
            super::construction_frontier::ConstructionFrontier::new(program, local.type_reference),
        ));
    }

    pub(super) fn is_reserved(&self, root: PlaceRoot) -> bool {
        self.constructing
            .iter()
            .any(|(candidate, _)| *candidate == root)
    }

    pub(super) fn is_present(&self, root: PlaceRoot, path: &[PlaceSegment]) -> bool {
        !self
            .excluded
            .iter()
            .any(|(candidate, removed)| *candidate == root && path.starts_with(removed))
            && self
                .constructing
                .iter()
                .find(|(candidate, _)| *candidate == root)
                .is_none_or(|(_, frontier)| frontier.contains(path))
    }

    pub(super) fn cleanup_paths(
        &self,
        root: PlaceRoot,
    ) -> Option<impl Iterator<Item = &[PlaceSegment]>> {
        self.constructing
            .iter()
            .find(|(candidate, _)| *candidate == root)
            .map(|(_, frontier)| {
                frontier
                    .established
                    .iter()
                    .rev()
                    .map(Vec::as_slice)
                    .filter(move |path| self.is_present(root, path))
            })
    }

    pub(super) fn advance(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        events: &[FlowPermissionEventFact],
        segments: &Arena<PlaceSegment>,
    ) {
        for event in events
            .iter()
            .filter(|event| event.access == PermissionAccess::Owned)
        {
            self.apply(program, event, segments.span_or_empty(event.segments));
        }
    }

    pub(super) fn disposal_paths(
        &self,
        root: PlaceRoot,
        path: &[PlaceSegment],
    ) -> Vec<Vec<PlaceSegment>> {
        if self.is_present(root, path) {
            return vec![path.to_vec()];
        }
        self.cleanup_paths(root)
            .into_iter()
            .flatten()
            .filter(|child| child.starts_with(path))
            .map(<[PlaceSegment]>::to_vec)
            .collect()
    }

    pub(super) fn observe(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        events: &[FlowPermissionEventFact],
        segments: &Arena<PlaceSegment>,
        output: &mut ContextualCleanupFacts,
        exit: FlowClaimJoinExit,
        establishments: &[StorageEstablishment],
    ) {
        for (event_ordinal, event) in events.iter().enumerate() {
            if event.access != PermissionAccess::Owned {
                continue;
            }
            // Dispose the old occurrence with its old exclusions before the
            // receiving write clears them. Apply a whole write only once:
            // later claim events may exclude absent conditional payloads.
            for establishment in establishments
                .iter()
                .filter(|establishment| establishment.event_ordinal == event_ordinal)
            {
                debug_assert_eq!(event.kind, PermissionEventKind::Establish);
                debug_assert_eq!(event.root, establishment.destination.root);
                self.establish_storage(
                    program,
                    establishment.destination.root,
                    &establishment.destination.segments,
                );
            }
            let path = segments.span_or_empty(event.segments);
            if event.kind == PermissionEventKind::AffineDrop
                || (event.kind == PermissionEventKind::Consume
                    && event.source == PermissionEventSource::StateExit)
            {
                let exclusions = self
                    .excluded
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
                    exit,
                    root: event.root,
                    disposed_type: typed_trees::types::TypeReferenceHandle::invalid(),
                    path: event.segments,
                    exclusions: output.exclusions.insert_many(exclusions),
                });
            }
            self.apply(program, event, path);
        }
    }

    fn apply(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        event: &FlowPermissionEventFact,
        path: &[PlaceSegment],
    ) {
        match event.kind {
            PermissionEventKind::Establish | PermissionEventKind::Restore => {
                // A strict child cannot revive its consumed containing owner.
                self.excluded
                    .retain(|(root, removed)| *root != event.root || !removed.starts_with(path));
                // A conditional claim with no live establishment denotes an
                // absent payload, not a dead value available for cleanup.
                if event.multiplicity == Multiplicity::Linear && !event.obligation_live {
                    self.remove(event.root, path);
                }
                if !self
                    .excluded
                    .iter()
                    .any(|(root, removed)| *root == event.root && path.starts_with(removed))
                    && let Some((_, frontier)) = self
                        .constructing
                        .iter_mut()
                        .find(|(root, _)| *root == event.root)
                {
                    frontier.establish(program, path);
                }
            }
            PermissionEventKind::Transfer
            | PermissionEventKind::Consume
            | PermissionEventKind::AffineDrop => {
                if let Some((_, frontier)) = self
                    .constructing
                    .iter_mut()
                    .find(|(root, _)| *root == event.root)
                {
                    frontier.retire(path);
                }
                self.remove(event.root, path);
            }
        }
    }

    fn remove(&mut self, root: PlaceRoot, path: &[PlaceSegment]) {
        if self
            .excluded
            .iter()
            .any(|(prior_root, prior)| *prior_root == root && path.starts_with(prior))
        {
            return;
        }
        self.excluded
            .retain(|(prior_root, prior)| *prior_root != root || !prior.starts_with(path));
        self.excluded.push((root, path.to_vec()));
    }

    /// A captured owned move transfers its selected storage along with the
    /// claim frontier. Descendant claim events alone cannot record a whole
    /// carrier transfer: retaining that home would dispose it on the edge.
    pub(super) fn transfer_storage(&mut self, root: PlaceRoot, path: &[PlaceSegment]) {
        self.remove(root, path);
    }

    /// Claim events name leaves; the successful write also establishes its
    /// containing storage region. A child write never clears a removed parent.
    fn establish_storage(
        &mut self,
        program: &dyn validation::CheckingInput,
        root: PlaceRoot,
        path: &[PlaceSegment],
    ) {
        self.excluded
            .retain(|(candidate, removed)| *candidate != root || !removed.starts_with(path));
        if let Some((_, frontier)) = self
            .constructing
            .iter_mut()
            .find(|(candidate, _)| *candidate == root)
        {
            frontier.establish(program, path);
        }
    }
}
