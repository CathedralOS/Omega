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
    // Linear locals whose whole storage is currently a zero image: present
    // zero-filled bits that established no value and owe no obligation
    // (ownership.md, "Establishment and erasure"). The first assignment
    // retires the image as a hook-free whole discard.
    zero_images: Vec<PlaceRoot>,
}

/// A receiving write's storage extent, paired with its first claim event in
/// the current evaluation. Claim events name leaves, not the containing write.
pub(super) struct StorageEstablishment {
    pub(super) event_ordinal: usize,
    pub(super) destination: validation::CanonicalPlace,
}

impl RemainingCustody {
    /// Ordinary post-selection availability is the intersection. Selected
    /// cleanup retains the exact skipped-path frontier separately; this join
    /// never turns possible custody into unconditional disposal authority.
    pub(super) fn join(&mut self, sibling: &Self) {
        for (root, path) in &sibling.excluded {
            self.remove(*root, path);
        }
        self.zero_images
            .retain(|root| sibling.zero_images.contains(root));
        for (root, frontier) in &mut self.constructing {
            if let Some((_, other)) = sibling
                .constructing
                .iter()
                .find(|(candidate, _)| candidate == root)
            {
                let mut common = Vec::new();
                for path in &frontier.established {
                    if other.contains(path) {
                        common.push(path.clone());
                    } else {
                        common.extend(
                            other
                                .established
                                .iter()
                                .filter(|child| child.starts_with(path))
                                .cloned(),
                        );
                    }
                }
                frontier.established = common;
            }
        }
    }

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
            || !super::construction_frontier::transparent_storage(program, local.type_reference)
            || !super::construction_frontier::requires_owned_establishment(
                program,
                local.type_reference,
            )
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

    /// Whether the reserved root is a whole value once `later` establishments
    /// join this custody's frontier: the retained path of a selected transfer
    /// sees both the pre-transfer frontier and what was stored afterwards.
    /// A root that is not under construction is already whole.
    pub(super) fn completes_with(
        &self,
        program: &dyn ::validation::CheckingInput,
        root: PlaceRoot,
        later: &[Vec<PlaceSegment>],
    ) -> bool {
        let Some((_, frontier)) = self
            .constructing
            .iter()
            .find(|(candidate, _)| *candidate == root)
        else {
            return true;
        };
        let mut merged = frontier.clone();
        for path in later {
            merged.establish(program, path);
        }
        merged.established.iter().any(Vec::is_empty)
    }

    /// The reserved root's established subtrees in establishment order, or
    /// none for a root that is not under construction.
    pub(super) fn established_paths(&self, root: PlaceRoot) -> &[Vec<PlaceSegment>] {
        self.constructing
            .iter()
            .find(|(candidate, _)| *candidate == root)
            .map_or(&[], |(_, frontier)| frontier.established.as_slice())
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

    /// An allocated, still-owned home must retire even when no semantic
    /// cleanup-bearing subtree remains. Initialized unrestricted fields do
    /// not create disposal work. This is exit geometry, not a persistent removal
    /// that could prevent a later initializing write.
    pub(super) fn needs_empty_retirement(
        &self,
        program: &dyn ::validation::CheckingInput,
        root: PlaceRoot,
    ) -> bool {
        !self
            .excluded
            .iter()
            .any(|(candidate, path)| *candidate == root && path.is_empty())
            && self
                .constructing
                .iter()
                .find(|(candidate, _)| *candidate == root)
                .is_some_and(|(_, frontier)| {
                    frontier
                        .established
                        .iter()
                        .filter(|path| self.is_present(root, path))
                        .all(|path| frontier.cleanup_free(program, path))
                })
    }

    pub(super) fn removed_paths<'a>(
        &'a self,
        program: &dyn ::validation::CheckingInput,
        root: PlaceRoot,
        path: &[PlaceSegment],
    ) -> Vec<&'a [PlaceSegment]> {
        if path.is_empty() && self.needs_empty_retirement(program, root) {
            return vec![&[]];
        }
        self.excluded
            .iter()
            .filter_map(|(candidate, removed)| {
                (*candidate == root)
                    .then(|| removed.strip_prefix(path))
                    .flatten()
            })
            .collect()
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

    /// `observe` for an ordinary statement edge, where every root dies in
    /// its declared order.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn observe_ordinary(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        events: &[FlowPermissionEventFact],
        segments: &Arena<PlaceSegment>,
        output: &mut ContextualCleanupFacts,
        exit: FlowClaimJoinExit,
        establishments: &[StorageEstablishment],
        selected: bool,
    ) {
        self.observe(
            program,
            events,
            segments,
            output,
            exit,
            establishments,
            selected,
            &[],
        );
    }

    /// Replay the statement from its entry custody, joining expression paths
    /// before its receiving write. A conditional restoration cannot erase a
    /// sibling's removal merely because its event appears later in the roster.
    pub(super) fn observe_evaluation(
        &mut self,
        program: &dyn validation::CheckingInput,
        routes: &super::selected_evaluation::ExpressionRoutes,
        calls: &[checked_trees::FlowCallFact],
        events: &[FlowPermissionEventFact],
        segments: &Arena<PlaceSegment>,
        output: &mut ContextualCleanupFacts,
        establishments: &[StorageEstablishment],
    ) -> Result<(), diagnostics::Diagnostic> {
        let mut selected = super::selected_evaluation::SelectedEvaluation::new();
        let join = |current: &mut Self, sibling: &Self| {
            current.join(sibling);
            Ok(())
        };
        for (ordinal, event) in events.iter().enumerate() {
            selected.enter(routes.event(event, calls), self, join)?;
            let receiving = establishments
                .iter()
                .filter(|establishment| establishment.event_ordinal == ordinal)
                .map(|establishment| StorageEstablishment {
                    event_ordinal: 0,
                    destination: establishment.destination.clone(),
                })
                .collect::<Vec<_>>();
            self.observe_ordinary(
                program,
                std::slice::from_ref(event),
                segments,
                output,
                FlowClaimJoinExit::default(),
                &receiving,
                false,
            );
        }
        selected.enter(&[], self, join)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn observe(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        events: &[FlowPermissionEventFact],
        segments: &Arena<PlaceSegment>,
        output: &mut ContextualCleanupFacts,
        exit: FlowClaimJoinExit,
        establishments: &[StorageEstablishment],
        selected: bool,
        establishment_ordered_roots: &[PlaceRoot],
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
            // A containing death is published before its exact child claim
            // receipts. Those receipts retire claims, but must not schedule
            // the same recursive hooks a second time below the dead region.
            if event.kind == PermissionEventKind::AffineDrop
                || (event.kind == PermissionEventKind::Consume
                    && (event.source == PermissionEventSource::StateExit
                        || (selected
                            && matches!(event.source, PermissionEventSource::Statement { .. })))
                    && self.is_present(event.root, path))
            {
                // A zero image dies whole and hook-free; it has no moved or
                // absent subtree to exclude and no empty-home retirement.
                let zero_image = path.is_empty() && self.zero_images.contains(&event.root);
                let mut exclusions = if zero_image {
                    Vec::new()
                } else {
                    self.excluded
                        .iter()
                        .filter(|(root, _)| *root == event.root)
                        .map(|(root, path)| Place {
                            root: *root,
                            segments: output.segments.insert_many(path.iter().copied()),
                        })
                        .collect::<Vec<_>>()
                };
                if !zero_image
                    && path.is_empty()
                    && self.needs_empty_retirement(program, event.root)
                {
                    exclusions.clear();
                    exclusions.push(Place {
                        root: event.root,
                        segments: Default::default(),
                    });
                }
                // A root that stays under construction at this edge, on every
                // path, has no folded whole value: the producer ordered its
                // deaths by establishment (`SelectedCleanup::finish`).
                let establishment_ordered = establishment_ordered_roots.contains(&event.root);
                let disposition = output.dispositions.append(ContextualCleanupDisposition {
                    machine: event.machine_symbol,
                    state: event.state_symbol,
                    source: event.source,
                    exit,
                    root: event.root,
                    disposed_type: typed_trees::types::TypeReferenceHandle::invalid(),
                    path: event.segments,
                    exclusions: output.exclusions.insert_many(exclusions),
                    establishment_ordered,
                    zero_image,
                });
                if selected {
                    output.selected_dispositions.append(disposition);
                }
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
                // absent payload, not a dead value available for cleanup. The
                // whole root is different: a linear local's obligation-free
                // whole write is its zero image, present storage that the
                // first assignment retires (`permission_production`).
                if event.multiplicity == Multiplicity::Linear && !event.obligation_live {
                    if path.is_empty() {
                        if !self.zero_images.contains(&event.root) {
                            self.zero_images.push(event.root);
                        }
                    } else {
                        self.remove(event.root, path);
                    }
                } else if path.is_empty() {
                    self.zero_images.retain(|root| *root != event.root);
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
                if path.is_empty() {
                    self.zero_images.retain(|root| *root != event.root);
                }
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

    /// A checked normal-continuation disposition retires the containing home,
    /// even when its permission receipts name only contained claims.
    pub(super) fn retire_storage(
        &mut self,
        root: PlaceRoot,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        source: PermissionEventSource,
        output: &mut ContextualCleanupFacts,
    ) {
        // A completed partial transfer can leave no residual region. Retain
        // its source death with the whole region excluded: this authorizes
        // home retirement, never a second disposal of the moved value.
        if !output.dispositions.iter().any(|(_, row)| {
            row.machine == machine && row.state == state && row.source == source && row.root == root
        }) {
            let exclusions = output.exclusions.insert_many([Place {
                root,
                segments: Default::default(),
            }]);
            output.dispositions.append(ContextualCleanupDisposition {
                machine,
                state,
                source,
                root,
                exit: FlowClaimJoinExit::default(),
                disposed_type: Default::default(),
                path: Default::default(),
                exclusions,
                establishment_ordered: false,
                zero_image: false,
            });
        }
        self.remove(root, &[]);
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
