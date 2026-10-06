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
use language_semantics::{PermissionAccess, PermissionEventKind, PermissionEventSource};

#[derive(Clone, Default)]
pub(crate) struct RemainingCustody {
    transferred: Vec<(PlaceRoot, Vec<PlaceSegment>)>,
    constructing: Vec<(
        PlaceRoot,
        super::construction_frontier::ConstructionFrontier,
    )>,
}

impl RemainingCustody {
    /// Reserve a deferred owned local without inventing a valid whole value.
    pub(super) fn reserve(
        &mut self,
        program: &typed_trees::TypedTrees,
        statement: &typed_trees::statement::StatementNode,
    ) {
        let typed_trees::statement::StatementNode::LocalData(local) = statement else {
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
            .transferred
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
        program: &typed_trees::TypedTrees,
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
        program: &typed_trees::TypedTrees,
        events: &[FlowPermissionEventFact],
        segments: &Arena<PlaceSegment>,
        output: &mut ContextualCleanupFacts,
        exit: FlowClaimJoinExit,
    ) {
        for event in events {
            if event.access != PermissionAccess::Owned {
                continue;
            }
            let path = segments.span_or_empty(event.segments);
            if event.kind == PermissionEventKind::AffineDrop
                || (event.kind == PermissionEventKind::Consume
                    && event.source == PermissionEventSource::StateExit)
            {
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
        program: &typed_trees::TypedTrees,
        event: &FlowPermissionEventFact,
        path: &[PlaceSegment],
    ) {
        match event.kind {
            PermissionEventKind::Establish => {
                // A strict child cannot revive its consumed containing owner.
                self.transferred
                    .retain(|(root, removed)| *root != event.root || !removed.starts_with(path));
                if !self
                    .transferred
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
