//! Exact remaining custody at a selected cleanup occurrence.
//!
//! This is a projection of the permission producer's occurrence order, not a
//! second control-flow analysis. The producer forks this state alongside its
//! ordinary ownership state for each outgoing edge.
mod case_exclusions;
use case_exclusions::CaseExclusions;

use checked_trees::{
    ContextualCleanupDisposition, ContextualCleanupExcludedRegion, ContextualCleanupExclusionCause,
    ContextualCleanupFacts, FlowClaimJoinExit, FlowPermissionEventFact,
};
use facts::{Place, PlaceRoot, PlaceSegment};
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionEventKind, PermissionEventSource,
};

#[derive(Clone, Default)]
pub(crate) struct RemainingCustody {
    // Moved regions and conditional claim regions that were never established
    // both lie outside the owner's cleanup complement.
    excluded: Vec<ExcludedStorage>,
    // Tag observations are facts, not transfers. They intersect at joins and
    // are invalidated by writes; unioning them can erase every live alternative.
    inactive_cases: CaseExclusions,
    storage_frontiers: Vec<(
        PlaceRoot,
        super::construction_frontier::ConstructionFrontier,
    )>,
    // Linear locals whose whole storage is currently a zero image: present
    // zero-filled bits that established no value and owe no obligation
    // (ownership.md, "Establishment and erasure"). The first assignment
    // retires the image as a hook-free whole discard.
    zero_images: Vec<PlaceRoot>,
}

/// Cleanup normalizes absent regions by containment. Retain the actual
/// consumed subregion separately as diagnostic provenance: coalescing an
/// unestablished ancestor must not claim that ancestor itself was transferred.
#[derive(Clone)]
struct ExcludedStorage {
    root: PlaceRoot,
    path: Vec<PlaceSegment>,
    consumed_path: Option<Vec<PlaceSegment>>,
    cause: ContextualCleanupExclusionCause,
}

/// A receiving write's storage extent, paired with its first claim event in
/// the current evaluation. Claim events name leaves, not the containing write.
pub(super) struct StorageEstablishment {
    pub(super) event_ordinal: usize,
    pub(super) destination: validation::CanonicalPlace,
}

/// A source evaluation position, distinct even when it emits no claim event.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CustodyCut {
    pub(crate) occurrence_ordinal: usize,
    pub(crate) permission_prefix: usize,
}

/// A completed invocation's physical write frontier at its exact event cut.
/// Unknown writes detach historical source associations; they never grant
/// replacement or transfer authority.
pub(super) struct StorageBindingChange {
    pub(super) cut: CustodyCut,
    pub(super) source: PermissionEventSource,
    pub(super) evaluation: checked_trees::FlowPermissionEvaluation,
    pub(super) route: Vec<crate::checks::ownership_occurrences::EvaluationBranch>,
    pub(super) writes: Option<Vec<validation::CanonicalPlace>>,
}

/// A selected move's exact pre-transfer custody, captured by the producer.
/// Permission events alone omit call invalidations and storage-only changes.
pub(crate) struct TransferCustody {
    pub(crate) occurrence_ordinal: usize,
    pub(crate) events: std::ops::Range<usize>,
    pub(crate) places: Vec<super::linear_obligations::LinearPlace>,
    pub(crate) remaining: RemainingCustody,
}

impl RemainingCustody {
    /// Compare the same occurrence after its exact receipt transfers. Holes
    /// are a set; construction order and case evidence remain exact.
    pub(super) fn same_storage_after_transfers(
        &self,
        other: &Self,
        root: PlaceRoot,
        paths: &[Vec<PlaceSegment>],
    ) -> bool {
        let mut left = self.clone();
        let mut right = other.clone();
        for path in paths {
            left.transfer_storage(root, path);
            right.transfer_storage(root, path);
        }
        left.same_storage_custody(&right, root)
    }

    pub(super) fn same_storage_custody(&self, other: &Self, root: PlaceRoot) -> bool {
        let equal_removal = |left: &ExcludedStorage, right: &ExcludedStorage| {
            left.root == right.root && left.path == right.path && left.cause == right.cause
        };
        self.excluded
            .iter()
            .filter(|entry| entry.root == root)
            .all(|entry| {
                other
                    .excluded
                    .iter()
                    .any(|other| equal_removal(entry, other))
            })
            && other
                .excluded
                .iter()
                .filter(|entry| entry.root == root)
                .all(|entry| {
                    self.excluded
                        .iter()
                        .any(|other| equal_removal(entry, other))
                })
            && self
                .inactive_cases
                .iter()
                .filter(|entry| entry.root == root)
                .eq(other
                    .inactive_cases
                    .iter()
                    .filter(|entry| entry.root == root))
            && self
                .storage_frontiers
                .iter()
                .filter(|(candidate, _)| *candidate == root)
                .eq(other
                    .storage_frontiers
                    .iter()
                    .filter(|(candidate, _)| *candidate == root))
            && self.zero_images.contains(&root) == other.zero_images.contains(&root)
    }

    /// A fresh construction vacancy is distinct from previously moved storage.
    pub(super) fn is_construction_vacancy(&self, root: PlaceRoot, path: &[PlaceSegment]) -> bool {
        self.storage_frontiers
            .iter()
            .any(|(candidate, frontier)| *candidate == root && !frontier.contains(path))
            && !self.excluded.iter().any(|removed| {
                removed.root == root
                    && (path.starts_with(&removed.path) || removed.path.starts_with(path))
            })
            && !self.inactive_cases.iter().any(|removed| {
                removed.root == root
                    && (path.starts_with(&removed.segments) || removed.segments.starts_with(path))
            })
    }

    pub(super) fn for_state(
        program: &dyn validation::CheckingInput,
        state: &typed_trees::state::State,
    ) -> Self {
        let mut custody = Self::default();
        for parameter in program.state_parameters(state) {
            if !parameter.relevance.is_erased()
                && validation::reference_shell(program, parameter.type_reference).is_none()
                && program.type_multiplicity(parameter.type_reference) != Multiplicity::Unrestricted
            {
                custody.storage_frontiers.push((
                    PlaceRoot::Symbol(parameter.symbol),
                    super::construction_frontier::ConstructionFrontier::completed(
                        program,
                        parameter.type_reference,
                    ),
                ));
            }
        }
        custody
    }

    /// Ordinary post-selection availability is the intersection. Selected
    /// cleanup retains the exact skipped-path frontier separately; this join
    /// never turns possible custody into unconditional disposal authority.
    pub(super) fn join(&mut self, sibling: &Self) {
        // Availability intersects by excluding either path's unavailable
        // regions. Positive absence evidence is stronger: it must name the
        // same exact region on both paths before it can survive the join.
        let common_unestablished = sibling
            .excluded
            .iter()
            .map(|right| {
                right.cause == ContextualCleanupExclusionCause::Unestablished
                    && self.excluded.iter().any(|left| {
                        left.root == right.root
                            && left.path == right.path
                            && left.cause == ContextualCleanupExclusionCause::Unestablished
                    })
            })
            .collect::<Vec<_>>();
        for left in &mut self.excluded {
            if left.cause == ContextualCleanupExclusionCause::Unestablished
                && !sibling.excluded.iter().any(|right| {
                    right.root == left.root
                        && right.path == left.path
                        && right.cause == ContextualCleanupExclusionCause::Unestablished
                })
            {
                left.cause = ContextualCleanupExclusionCause::Unknown;
            }
        }
        for (removed, common) in sibling.excluded.iter().zip(common_unestablished) {
            let cause =
                if removed.cause == ContextualCleanupExclusionCause::Unestablished && !common {
                    ContextualCleanupExclusionCause::Unknown
                } else {
                    removed.cause
                };
            self.remove(
                removed.root,
                &removed.path,
                cause,
                removed.consumed_path.as_deref(),
            );
        }
        self.inactive_cases
            .retain(|observation| sibling.inactive_cases.contains(observation));
        self.zero_images
            .retain(|root| sibling.zero_images.contains(root));
        self.storage_frontiers.retain(|(root, _)| {
            sibling
                .storage_frontiers
                .iter()
                .any(|(other, _)| other == root)
        });
        for (root, frontier) in &mut self.storage_frontiers {
            if let Some((_, other)) = sibling
                .storage_frontiers
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
        self.storage_frontiers.push((
            PlaceRoot::Symbol(local.symbol),
            super::construction_frontier::ConstructionFrontier::new(program, local.type_reference),
        ));
    }

    pub(super) fn has_storage_frontier(&self, root: PlaceRoot) -> bool {
        self.storage_frontiers
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
        receiving_writes: &[Vec<PlaceSegment>],
    ) -> bool {
        let Some((_, frontier)) = self
            .storage_frontiers
            .iter()
            .find(|(candidate, _)| *candidate == root)
        else {
            return true;
        };
        frontier.completes_with(program, later, receiving_writes)
    }

    /// The authenticated storage root's established subtrees in establishment
    /// order, or none when no formation frontier is retained.
    pub(super) fn established_paths(&self, root: PlaceRoot) -> &[Vec<PlaceSegment>] {
        self.storage_frontiers
            .iter()
            .find(|(candidate, _)| *candidate == root)
            .map_or(&[], |(_, frontier)| frontier.established.as_slice())
    }

    pub(super) fn is_present(&self, root: PlaceRoot, path: &[PlaceSegment]) -> bool {
        !self
            .excluded
            .iter()
            .any(|removed| removed.root == root && path.starts_with(&removed.path))
            && !self
                .inactive_cases
                .iter()
                .any(|inactive| inactive.root == root && path.starts_with(&inactive.segments))
            && self
                .storage_frontiers
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
            .any(|removed| removed.root == root && removed.path.is_empty())
            && self
                .storage_frontiers
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
        self.removed_regions(program, root, path)
            .into_iter()
            .map(|(path, _)| path)
            .collect()
    }

    pub(super) fn removed_regions<'a>(
        &'a self,
        program: &dyn ::validation::CheckingInput,
        root: PlaceRoot,
        path: &[PlaceSegment],
    ) -> Vec<(&'a [PlaceSegment], ContextualCleanupExclusionCause)> {
        if path.is_empty() && self.needs_empty_retirement(program, root) {
            return vec![(&[], ContextualCleanupExclusionCause::Unknown)];
        }
        self.excluded
            .iter()
            .filter_map(|removed| {
                (removed.root == root)
                    .then(|| {
                        removed
                            .path
                            .strip_prefix(path)
                            .map(|path| (path, removed.cause))
                    })
                    .flatten()
            })
            .collect()
    }

    pub(super) fn inactive_case_paths<'a>(
        &'a self,
        root: PlaceRoot,
        path: &[PlaceSegment],
    ) -> Vec<&'a [PlaceSegment]> {
        self.inactive_cases
            .iter()
            .filter_map(|inactive| {
                (inactive.root == root)
                    .then(|| inactive.segments.strip_prefix(path))
                    .flatten()
            })
            .collect()
    }

    pub(super) fn cleanup_paths(
        &self,
        root: PlaceRoot,
    ) -> Option<impl Iterator<Item = &[PlaceSegment]>> {
        self.storage_frontiers
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
        segments: &arena::PagedArenaSet<PlaceSegment>,
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
        program: &dyn ::validation::CheckingInput,
        root: PlaceRoot,
        path: &[PlaceSegment],
    ) -> Vec<Vec<PlaceSegment>> {
        // Preserve the allocated home's boundary when its remaining children
        // owe no value cleanup. Expanding that boundary would turn retirement
        // back into disposal of raw or unrestricted storage fields.
        if path.is_empty() && self.needs_empty_retirement(program, root) {
            return vec![Vec::new()];
        }
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
        segments: &arena::PagedArenaSet<PlaceSegment>,
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
        segments: &arena::PagedArenaSet<PlaceSegment>,
        output: &mut ContextualCleanupFacts,
        establishments: &[StorageEstablishment],
    ) -> Result<(), diagnostics::Diagnostic> {
        if events.is_empty() {
            return Ok(());
        }
        let mut selected = super::selected_evaluation::SelectedEvaluation::new();
        let join = |_branch, current: &mut Self, sibling: &Self| {
            current.join(sibling);
            Ok(())
        };
        for (ordinal, event) in events.iter().enumerate() {
            selected.enter(routes.event(event, calls), self, |_, _| Ok(()), join)?;
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
        selected.enter(&[], self, |_, _| Ok(()), join)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn observe(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        events: &[FlowPermissionEventFact],
        segments: &arena::PagedArenaSet<PlaceSegment>,
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
                output.storage_establishments.append(
                    checked_trees::ContextualStorageEstablishment {
                        machine: event.machine_symbol,
                        state: event.state_symbol,
                        source: event.source,
                        destination: Place {
                            root: establishment.destination.root,
                            segments: output
                                .segments
                                .insert_many(establishment.destination.segments.iter().copied()),
                        },
                    },
                );
                self.establish_storage(program, &establishment.destination, event);
            }
            let path = segments.span_or_empty(event.segments);
            // A containing death is published before its exact child claim
            // receipts. Those receipts retire claims, but must not schedule
            // the same recursive hooks a second time below the dead region.
            let whole_owned_disposition = event.kind == PermissionEventKind::AffineDrop
                && self.is_present(event.root, path)
                && output
                    .dispositions
                    .iter()
                    .filter(|(_, row)| {
                        row.machine == event.machine_symbol
                            && row.state == event.state_symbol
                            && row.source == event.source
                            && row.completion == event.expression
                            && row.exit == exit
                            && row.root == event.root
                            && row.disposed_type.is_valid()
                            && row.path.is_empty()
                            && row.exclusions.is_empty()
                            && !row.zero_image
                            && !row.establishment_ordered
                    })
                    .take(2)
                    .count()
                    == 1;
            if (event.kind == PermissionEventKind::AffineDrop && !whole_owned_disposition)
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
                        .filter(|removed| removed.root == event.root)
                        .map(|removed| ContextualCleanupExcludedRegion {
                            root: removed.root,
                            segments: output.segments.insert_many(removed.path.iter().copied()),
                            cause: removed.cause,
                        })
                        .collect::<Vec<_>>()
                };
                if !zero_image
                    && path.is_empty()
                    && self.needs_empty_retirement(program, event.root)
                {
                    exclusions.clear();
                    exclusions.push(ContextualCleanupExcludedRegion {
                        root: event.root,
                        segments: Default::default(),
                        cause: ContextualCleanupExclusionCause::Unknown,
                    });
                }
                // A root that stays under construction at this edge, on every
                // path, has no folded whole value: the producer ordered its
                // deaths by establishment (`SelectedCleanup::finish`).
                let establishment_ordered = establishment_ordered_roots.contains(&event.root);
                let inactive_cases = self
                    .inactive_cases
                    .iter()
                    .filter(|inactive| inactive.root == event.root)
                    .map(|inactive| ContextualCleanupExcludedRegion {
                        root: inactive.root,
                        cause: ContextualCleanupExclusionCause::InactiveCase,
                        segments: output
                            .segments
                            .insert_many(inactive.segments.iter().copied()),
                    })
                    .collect::<Vec<_>>();
                let disposition = output.dispositions.append(ContextualCleanupDisposition {
                    selection: Default::default(),
                    machine: event.machine_symbol,
                    state: event.state_symbol,
                    source: event.source,
                    exit,
                    completion: Default::default(),
                    completion_constraints: None,
                    root: event.root,
                    disposed_type: typed_trees::types::TypeReferenceHandle::invalid(),
                    path: event.segments,
                    exclusions: output.exclusions.insert_many(exclusions),
                    inactive_cases: output.exclusions.insert_many(inactive_cases),
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
                // A declaration's zero-case receipt excludes an absent
                // alternative; it does not partially move the containing
                // value. Preserve its whole cleanup and nominal prerequisites.
                if event.kind == PermissionEventKind::Establish
                    && event.multiplicity == Multiplicity::Linear
                    && !event.obligation_live
                    && let PermissionEventSource::Statement { statement_index } = event.source
                    && let Some(state) = program.state_by_symbol(event.state_symbol)
                    && let Some(local) = program
                        .statement_table
                        .statements(state.statement_nodes)
                        .get(statement_index)
                        .and_then(|statement| statement.as_local_data())
                    && event.root == PlaceRoot::Symbol(local.symbol)
                    && !local.initial_value.is_valid()
                    && !validation::type_requires_establishment(program, local.type_reference)
                    && let Some(inactive) = super::construction_frontier::zero_image_inactive_case(
                        program,
                        local.type_reference,
                        path,
                    )
                {
                    self.exclude_absent_case(&validation::CanonicalPlace {
                        root: event.root,
                        segments: inactive.to_vec(),
                    });
                    return;
                }
                self.invalidate_case_observations(program, event.root, path);
                // A strict child cannot revive its consumed containing owner.
                self.excluded.retain(|removed| {
                    removed.root != event.root || !removed.path.starts_with(path)
                });
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
                        // A receiving establishment positively reports that
                        // this typed claim region never held a value. Restore
                        // also uses this replay arm, but without a separately
                        // authenticated receiving proof it remains unknown.
                        let cause = if event.kind == PermissionEventKind::Establish {
                            ContextualCleanupExclusionCause::Unestablished
                        } else {
                            ContextualCleanupExclusionCause::Unknown
                        };
                        self.remove(event.root, path, cause, None);
                    }
                } else if path.is_empty() {
                    self.zero_images.retain(|root| *root != event.root);
                }
                if !self
                    .excluded
                    .iter()
                    .any(|removed| removed.root == event.root && path.starts_with(&removed.path))
                    && let Some((_, frontier)) = self
                        .storage_frontiers
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
                    .storage_frontiers
                    .iter_mut()
                    .find(|(root, _)| *root == event.root)
                {
                    frontier.retire(path);
                }
                self.remove(
                    event.root,
                    path,
                    ContextualCleanupExclusionCause::Consumed,
                    Some(path),
                );
            }
        }
    }

    /// The source schedule proved this whole case absent on this alternative.
    /// Preserve the containing home; this is not an owned transfer or death.
    pub(super) fn exclude_absent_case(&mut self, place: &validation::CanonicalPlace) {
        debug_assert!(matches!(
            place.segments.last(),
            Some(PlaceSegment::Case { .. })
        ));
        self.inactive_cases.insert(place);
    }

    fn invalidate_case_observations(
        &mut self,
        program: &dyn validation::CheckingInput,
        root: PlaceRoot,
        path: &[PlaceSegment],
    ) {
        for removed in &mut self.excluded {
            if removed.root == root
                && removed.cause == ContextualCleanupExclusionCause::Unestablished
                && crate::flow::canonical_place_segments_may_overlap(program, &removed.path, path)
            {
                removed.cause = ContextualCleanupExclusionCause::Unknown;
            }
        }
        self.inactive_cases.retain(|inactive| {
            inactive.root != root
                || !crate::flow::canonical_place_segments_may_overlap(
                    program,
                    &inactive.segments,
                    path,
                )
        });
    }

    /// Forget observations at the exact returned invocation before any owner
    /// cleanup. Borrowed writes can change tags without restoring a linear
    /// claim; unknown effects forget facts but never manufacture authority.
    pub(super) fn invalidate_call_observations(
        &mut self,
        program: &dyn validation::CheckingInput,
        writes: Option<&[validation::CanonicalPlace]>,
    ) {
        let Some(writes) = writes else {
            self.inactive_cases.clear();
            for removed in &mut self.excluded {
                if removed.cause == ContextualCleanupExclusionCause::Unestablished {
                    removed.cause = ContextualCleanupExclusionCause::Unknown;
                }
            }
            return;
        };
        for write in writes {
            self.invalidate_case_observations(program, write.root, &write.segments);
        }
    }

    fn remove(
        &mut self,
        root: PlaceRoot,
        path: &[PlaceSegment],
        cause: ContextualCleanupExclusionCause,
        consumed_path: Option<&[PlaceSegment]>,
    ) {
        if let Some(prior) = self
            .excluded
            .iter_mut()
            .find(|prior| prior.root == root && path.starts_with(&prior.path))
        {
            if prior.path != path || prior.cause != cause {
                prior.cause = ContextualCleanupExclusionCause::Unknown;
            }
            if prior.consumed_path.is_none() {
                prior.consumed_path = consumed_path.map(<[PlaceSegment]>::to_vec);
            }
            return;
        }
        let cause = if self.excluded.iter().any(|prior| {
            prior.root == root
                && prior.path.starts_with(path)
                && (prior.path != path || prior.cause != cause)
        }) {
            ContextualCleanupExclusionCause::Unknown
        } else {
            cause
        };
        let consumed_path = consumed_path.map(<[PlaceSegment]>::to_vec).or_else(|| {
            self.excluded.iter().find_map(|prior| {
                (prior.root == root && prior.path.starts_with(path))
                    .then(|| prior.consumed_path.clone())
                    .flatten()
            })
        });
        self.excluded
            .retain(|prior| prior.root != root || !prior.path.starts_with(path));
        self.excluded.push(ExcludedStorage {
            root,
            path: path.to_vec(),
            consumed_path,
            cause,
        });
    }

    /// Only a real transfer/disposal cause diagnoses consumption. A missing
    /// establishment or inactive tag alone remains a different source failure.
    pub(super) fn consumed_place(
        &self,
        root: PlaceRoot,
        path: &[PlaceSegment],
    ) -> Option<validation::CanonicalPlace> {
        self.excluded.iter().find_map(|removed| {
            let consumed = removed.consumed_path.as_deref()?;
            (removed.root == root && (path.starts_with(consumed) || consumed.starts_with(path)))
                .then(|| validation::CanonicalPlace {
                    root,
                    segments: consumed.to_vec(),
                })
        })
    }

    /// A captured owned move transfers its selected storage along with the
    /// claim frontier. Descendant claim events alone cannot record a whole
    /// carrier transfer: retaining that home would dispose it on the edge.
    pub(super) fn transfer_storage(&mut self, root: PlaceRoot, path: &[PlaceSegment]) {
        self.remove(
            root,
            path,
            ContextualCleanupExclusionCause::Consumed,
            Some(path),
        );
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
            let exclusions = output
                .exclusions
                .insert_many([ContextualCleanupExcludedRegion {
                    root,
                    segments: Default::default(),
                    cause: ContextualCleanupExclusionCause::Consumed,
                }]);
            output.dispositions.append(ContextualCleanupDisposition {
                selection: Default::default(),
                machine,
                state,
                source,
                root,
                exit: FlowClaimJoinExit::default(),
                completion: Default::default(),
                completion_constraints: None,
                disposed_type: Default::default(),
                path: Default::default(),
                exclusions,
                inactive_cases: Default::default(),
                establishment_ordered: false,
                zero_image: false,
            });
        }
        self.remove(
            root,
            &[],
            ContextualCleanupExclusionCause::Consumed,
            Some(&[]),
        );
    }

    /// Claim events name leaves; the successful write also establishes its
    /// containing storage region. A child write never clears a removed parent.
    fn establish_storage(
        &mut self,
        program: &dyn validation::CheckingInput,
        destination: &validation::CanonicalPlace,
        event: &FlowPermissionEventFact,
    ) {
        let root = destination.root;
        let path = destination.segments.as_slice();
        let state = event.state_symbol;
        let zero_image = path.is_empty()
            && event.segments.is_empty()
            && event.multiplicity == Multiplicity::Linear
            && !event.obligation_live;
        self.invalidate_case_observations(program, root, path);
        self.excluded
            .retain(|removed| removed.root != root || !removed.path.starts_with(path));
        // Only a whole completed write can introduce a new storage frontier.
        // Child writes update an already reserved/established one; they cannot
        // infer that an otherwise unavailable aggregate was formed.
        if path.is_empty()
            && !zero_image
            && !self
                .storage_frontiers
                .iter()
                .any(|(candidate, _)| *candidate == root)
            && let Some(symbol) = root.as_symbol()
            && let Some(state) = program.state_by_symbol(state)
            && let Some((_, local)) = program.state_local_declaration(state, symbol)
            && !local.relevance.is_erased()
        {
            self.storage_frontiers.push((
                root,
                super::construction_frontier::ConstructionFrontier::completed(
                    program,
                    local.type_reference,
                ),
            ));
        }
        if let Some((_, frontier)) = self
            .storage_frontiers
            .iter_mut()
            .find(|(candidate, _)| *candidate == root)
            && !zero_image
        {
            frontier.establish(program, path);
        }
    }
}
