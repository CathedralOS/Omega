/*
    Exact remaining custody at a selected cleanup occurrence.

    This is a projection of the permission producer's occurrence order, not a
    second control-flow analysis. The producer forks this state alongside its
    ordinary ownership state for each outgoing edge.
*/

mod case_exclusions {
    /*
        Invocation-local case observations in first-observation order.

        Permission replay forks the same authenticated exclusions at successive
        cuts. Share the immutable table and paths until a branch changes its facts;
        copying every structural path at each cut makes large checking bodies retain
        the same evidence many times. The table keeps its publication order and exact
        identity index together. Hashes only filter candidates: full structural
        equality decides collisions, and invalidation still visits every original
        observation exactly once in order.
    */
    use facts::{PlaceRoot, PlaceSegment};
    use std::hash::{Hash, Hasher};
    use std::sync::Arc;
    use symbols::{SymbolHasher, SymbolKeyMap};
    use validation::CanonicalPlace;

    #[derive(Clone)]
    struct CaseObservation {
        place: Arc<CanonicalPlace>,
        fingerprint: u64,
    }

    #[derive(Clone, Default)]
    struct CaseObservationTable {
        observations: Vec<CaseObservation>,
        by_identity: SymbolKeyMap<u64, usize>,
        /* How many observations name a range of array elements. */
        element_ranges: usize,
    }

    /*
        An inactive case of a zero-formed array is observed once for a range of
        elements: its path spells that range where one element's path spells its
        index. Such a path stands for the same case in every element of the range,
        so it is a prefix of an element's path when the index lies inside it.
    */
    fn observed_segment_admits(observed: PlaceSegment, queried: PlaceSegment) -> bool {
        observed == queried
            || matches!(
                (observed, queried),
                (
                    PlaceSegment::FixedRange { start, end },
                    PlaceSegment::FixedIndex { index },
                ) if start <= index && index < end
            )
    }

    fn names_element_range(path: &[PlaceSegment]) -> bool {
        path.iter()
            .any(|segment| matches!(segment, PlaceSegment::FixedRange { .. }))
    }

    /* The observed case path, or a path below it. */
    pub(in crate::checks::multiplicity) fn lies_under_observed_case(
        observed: &[PlaceSegment],
        path: &[PlaceSegment],
    ) -> bool {
        observed.len() <= path.len()
            && observed
                .iter()
                .zip(path)
                .all(|(observed, queried)| observed_segment_admits(*observed, *queried))
    }

    /* A path that contains the observed case path. */
    pub(in crate::checks::multiplicity) fn contains_observed_case(
        path: &[PlaceSegment],
        observed: &[PlaceSegment],
    ) -> bool {
        path.len() <= observed.len()
            && observed
                .iter()
                .zip(path)
                .all(|(observed, queried)| observed_segment_admits(*observed, *queried))
    }

    #[derive(Clone, Default)]
    pub(super) struct CaseExclusions {
        table: Arc<CaseObservationTable>,
    }

    impl CaseExclusions {
        fn observation_fingerprint(place: &CanonicalPlace) -> u64 {
            let mut hasher = SymbolHasher::default();
            place.hash(&mut hasher);
            hasher.finish()
        }

        fn identity_position(&self, fingerprint: u64, place: &CanonicalPlace) -> Option<usize> {
            let &first = self.table.by_identity.get(&fingerprint)?;
            if self.table.observations[first].place.as_ref() == place {
                return Some(first);
            }
            self.table
                .observations
                .iter()
                .position(|observation| observation.place.as_ref() == place)
        }

        pub(super) fn insert(&mut self, place: &CanonicalPlace) {
            let fingerprint = Self::observation_fingerprint(place);
            if self.identity_position(fingerprint, place).is_some() {
                return;
            }
            let table = Arc::make_mut(&mut self.table);
            table
                .by_identity
                .entry(fingerprint)
                .or_insert(table.observations.len());
            table.element_ranges += usize::from(names_element_range(&place.segments));
            table.observations.push(CaseObservation {
                place: Arc::new(place.clone()),
                fingerprint,
            });
        }

        pub(super) fn contains(&self, place: &CanonicalPlace) -> bool {
            self.identity_position(Self::observation_fingerprint(place), place)
                .is_some()
        }

        /*
            Query exact ancestors through the existing identity index, including
            the empty prefix. Membership keeps its complete collision comparisons.
        */
        pub(super) fn contains_prefix_of(&self, root: PlaceRoot, path: &[PlaceSegment]) -> bool {
            if self.table.observations.is_empty() {
                return false;
            }
            let mut query = CanonicalPlace {
                root,
                segments: path.to_vec(),
            };
            loop {
                if self.contains(&query) {
                    return true;
                }
                if query.segments.pop().is_none() {
                    break;
                }
            }
            self.table.element_ranges != 0
                && self.table.observations.iter().any(|observation| {
                    observation.place.root == root
                        && names_element_range(&observation.place.segments)
                        && lies_under_observed_case(&observation.place.segments, path)
                })
        }

        pub(super) fn iter(&self) -> impl Iterator<Item = &CanonicalPlace> {
            self.table
                .observations
                .iter()
                .map(|observation| observation.place.as_ref())
        }

        pub(super) fn retain(&mut self, mut keep: impl FnMut(&CanonicalPlace) -> bool) {
            let Some(first_removed) = self
                .table
                .observations
                .iter()
                .position(|observation| !keep(&observation.place))
            else {
                return;
            };
            let table = Arc::make_mut(&mut self.table);
            let mut position = 0;
            table.observations.retain(|observation| {
                let original = position;
                position += 1;
                original < first_removed || (original > first_removed && keep(&observation.place))
            });
            table.by_identity.clear();
            table.element_ranges = table
                .observations
                .iter()
                .filter(|observation| names_element_range(&observation.place.segments))
                .count();
            for (position, observation) in table.observations.iter().enumerate() {
                table
                    .by_identity
                    .entry(observation.fingerprint)
                    .or_insert(position);
            }
        }

        pub(super) fn clear(&mut self) {
            self.table = Arc::default();
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{CaseExclusions, CaseObservation, CaseObservationTable};
        use facts::{PlaceRoot, PlaceSegment};
        use std::sync::Arc;
        use symbols::SymbolHandle;
        use validation::CanonicalPlace;

        fn place(
            root_generation: u32,
            case_generation: u32,
            element_index: usize,
        ) -> CanonicalPlace {
            CanonicalPlace {
                root: PlaceRoot::Symbol(SymbolHandle::from_parts(3, root_generation)),
                segments: vec![
                    PlaceSegment::FixedIndex {
                        index: element_index,
                    },
                    PlaceSegment::Case {
                        variant: SymbolHandle::from_parts(7, case_generation),
                    },
                ],
            }
        }

        /*
            `root.table[2..6].kind/Case` observed once for four elements answers
            for a path through any of them and for no other element.
        */
        #[test]
        fn an_element_range_observation_covers_each_element_in_it() {
            let field = |index| PlaceSegment::Field {
                symbol: SymbolHandle::from_parts(index, 1),
            };
            let case = PlaceSegment::Case {
                variant: SymbolHandle::from_parts(9, 1),
            };
            let root = PlaceRoot::Symbol(SymbolHandle::from_parts(1, 1));
            let observed = CanonicalPlace {
                root,
                segments: vec![
                    field(2),
                    PlaceSegment::FixedRange { start: 2, end: 6 },
                    field(3),
                    case,
                ],
            };
            let mut exclusions = CaseExclusions::default();
            exclusions.insert(&observed);
            let through = |index| {
                vec![
                    field(2),
                    PlaceSegment::FixedIndex { index },
                    field(3),
                    case,
                    field(4),
                ]
            };
            assert!(exclusions.contains_prefix_of(root, &through(2)));
            assert!(exclusions.contains_prefix_of(root, &through(5)));
            assert!(!exclusions.contains_prefix_of(root, &through(6)));
            assert!(!exclusions.contains_prefix_of(root, &through(1)));
            assert!(!exclusions.contains_prefix_of(root, &through(3)[..3]));
            assert!(!exclusions.contains_prefix_of(
                PlaceRoot::Symbol(SymbolHandle::from_parts(7, 1)),
                &through(3)
            ));
            assert!(super::lies_under_observed_case(
                &observed.segments,
                &through(4)
            ));
            assert!(super::contains_observed_case(
                &through(4)[..2],
                &observed.segments
            ));
            assert!(!super::contains_observed_case(
                &through(7)[..2],
                &observed.segments
            ));
            exclusions.retain(|_| false);
            assert!(!exclusions.contains_prefix_of(root, &through(3)));
        }

        #[test]
        fn hash_collisions_do_not_join_structural_identities() {
            let places = [
                place(1, 1, 8),
                place(1, 2, 8),
                place(2, 1, 8),
                place(1, 1, 9),
            ];
            let fingerprint = 7;
            let mut exclusions = CaseExclusions {
                table: Arc::new(CaseObservationTable {
                    observations: places[..3]
                        .iter()
                        .map(|place| CaseObservation {
                            place: Arc::new(place.clone()),
                            fingerprint,
                        })
                        .collect(),
                    by_identity: [(fingerprint, 0)].into_iter().collect(),
                    element_ranges: 0,
                }),
            };
            for (position, place) in places[..3].iter().enumerate() {
                assert_eq!(
                    exclusions.identity_position(fingerprint, place),
                    Some(position)
                );
            }
            assert_eq!(exclusions.identity_position(fingerprint, &places[3]), None);
            exclusions.retain(|place| place != &places[1]);
            assert_eq!(exclusions.identity_position(fingerprint, &places[1]), None);
            assert_eq!(
                exclusions.identity_position(fingerprint, &places[2]),
                Some(1)
            );
            exclusions.retain(|place| place != &places[0]);
            assert_eq!(exclusions.identity_position(fingerprint, &places[0]), None);
            assert_eq!(
                exclusions.identity_position(fingerprint, &places[2]),
                Some(0)
            );
        }

        #[test]
        fn indexed_prefix_queries_match_linear_observations_after_mutation() {
            let exact = place(1, 1, 8);
            let mut descendant = exact.clone();
            descendant.segments.push(PlaceSegment::Field {
                symbol: SymbolHandle::from_parts(11, 1),
            });
            let mut ancestor = exact.clone();
            ancestor.segments.pop();
            let empty = CanonicalPlace {
                root: exact.root,
                segments: Vec::new(),
            };
            let mut anonymous = exact.clone();
            anonymous.root = PlaceRoot::Expression(arena::Handle::from_parts(3, 1));
            let queries = [
                exact.clone(),
                descendant,
                ancestor.clone(),
                empty.clone(),
                place(1, 1, 9),
                place(1, 2, 8),
                place(2, 1, 8),
                anonymous.clone(),
            ];
            let check = |observations: &CaseExclusions| {
                for query in &queries {
                    assert_eq!(
                        observations.contains_prefix_of(query.root, &query.segments),
                        observations.iter().any(|stored| {
                            stored.root == query.root
                                && query.segments.starts_with(&stored.segments)
                        }),
                        "query {query:?}",
                    );
                }
            };
            let mut observations = CaseExclusions::default();
            check(&observations);
            for stored in [&exact, &queries[5], &queries[6], &anonymous] {
                observations.insert(stored);
                check(&observations);
            }
            assert!(!observations.contains_prefix_of(ancestor.root, &ancestor.segments));
            assert!(!observations.contains_prefix_of(empty.root, &empty.segments));
            let mut branch = observations.clone();
            branch.retain(|stored| stored != &exact);
            check(&branch);
            check(&observations);
            branch.insert(&empty);
            check(&branch);
            assert!(branch.contains_prefix_of(empty.root, &empty.segments));
            assert!(branch.contains_prefix_of(queries[4].root, &queries[4].segments));
            branch.retain(|stored| stored != &empty);
            branch.insert(&exact);
            check(&branch);
            observations.clear();
            check(&observations);
            observations.insert(&anonymous);
            check(&observations);
            check(&branch);
        }

        #[test]
        fn repeated_cuts_preserve_first_observation_and_complete_identities() {
            let places = [
                place(2, 1, 8),
                place(1, 1, 4),
                place(1, 2, 4),
                place(1, 1, 8),
            ];
            for length in 0..=5 {
                for sequence in 0..places.len().pow(length) {
                    let mut digits = sequence;
                    let mut exclusions = CaseExclusions::default();
                    let mut expected = Vec::new();
                    let mut mixed = CaseExclusions::default();
                    let mut scratch = CanonicalPlace {
                        root: PlaceRoot::Unknown,
                        segments: Vec::new(),
                    };
                    for _ in 0..length {
                        let place = &places[digits % places.len()];
                        digits /= places.len();
                        exclusions.insert(place);
                        scratch.root = place.root;
                        scratch.segments.clone_from(&place.segments);
                        mixed.insert(&scratch);
                        scratch.segments.clear();
                        if !expected.contains(place) {
                            expected.push(place.clone());
                        }
                    }
                    assert_eq!(exclusions.iter().cloned().collect::<Vec<_>>(), expected);
                    assert_eq!(mixed.iter().cloned().collect::<Vec<_>>(), expected);
                    let mut branch = mixed.clone();
                    for place in &places {
                        assert_eq!(exclusions.contains(place), expected.contains(place));
                        assert_eq!(mixed.contains(place), expected.contains(place));
                    }
                    for place in &expected {
                        branch.insert(place);
                        assert!(Arc::ptr_eq(&mixed.table, &branch.table));
                    }
                    let added = place(9, 9, 99);
                    branch.insert(&added);
                    assert!(!Arc::ptr_eq(&mixed.table, &branch.table));
                    assert_eq!(mixed.iter().cloned().collect::<Vec<_>>(), expected);
                    scratch.root = added.root;
                    scratch.segments.clone_from(&added.segments);
                    let mut retained = CaseExclusions::default();
                    retained.insert(&scratch);
                    scratch.root = PlaceRoot::Unknown;
                    scratch.segments.clear();
                    assert_eq!(retained.iter().next(), Some(&added));
                }
            }
        }

        #[test]
        fn intersect_invalidate_and_reestablish_keep_the_surviving_order() {
            let places = [
                place(2, 1, 8),
                place(1, 1, 4),
                place(1, 2, 4),
                place(1, 1, 8),
            ];
            let mut exclusions = CaseExclusions::default();
            let mut sibling = CaseExclusions::default();
            for place in &places {
                exclusions.insert(place);
            }
            for place in [&places[3], &places[0], &places[2]] {
                sibling.insert(place);
            }
            exclusions.retain(|place| sibling.contains(place));
            assert_eq!(
                exclusions.iter().collect::<Vec<_>>(),
                vec![&places[0], &places[2], &places[3]]
            );
            exclusions.retain(|place| place != &places[2]);
            exclusions.retain(|_| true);
            exclusions.insert(&places[3]);
            exclusions.insert(&places[2]);
            assert_eq!(
                exclusions.iter().collect::<Vec<_>>(),
                vec![&places[0], &places[3], &places[2]]
            );
            assert!(!exclusions.contains(&places[1]));
            let mut branch = exclusions.clone();
            branch.retain(|place| place != &places[0]);
            branch.insert(&places[1]);
            assert_eq!(
                exclusions.iter().collect::<Vec<_>>(),
                vec![&places[0], &places[3], &places[2]]
            );
            exclusions.clear();
            exclusions.insert(&places[1]);
            branch.retain(|_| false);
            branch.insert(&places[0]);
            assert_eq!(exclusions.iter().collect::<Vec<_>>(), vec![&places[1]]);
            assert_eq!(branch.iter().collect::<Vec<_>>(), vec![&places[0]]);
        }

        #[test]
        fn repeated_deep_structural_paths_and_root_kinds_remain_distinct() {
            let mut exclusions = CaseExclusions::default();
            let mut expected = Vec::new();
            for element_index in (0..256).rev() {
                let mut owned = place(1, 1, element_index);
                owned.segments.splice(
                    0..0,
                    (0..16).map(|depth| PlaceSegment::Field {
                        symbol: SymbolHandle::from_parts(depth + 1, 1),
                    }),
                );
                let mut anonymous = owned.clone();
                anonymous.root = PlaceRoot::Expression(arena::Handle::from_parts(3, 1));
                let mut referent = owned.clone();
                referent.root = PlaceRoot::InvocationReferent {
                    parameter: SymbolHandle::from_parts(3, 1),
                    type_reference: arena::Handle::from_parts(5, 1),
                };
                for place in [owned, anonymous, referent] {
                    exclusions.insert(&place);
                    expected.push(place);
                }
            }
            for _ in 0..8 {
                for place in expected.iter().rev() {
                    exclusions.insert(place);
                }
            }
            assert_eq!(exclusions.iter().cloned().collect::<Vec<_>>(), expected);
            for place in &expected {
                assert!(exclusions.contains(place));
            }
        }

        #[test]
        fn compaction_matches_ordered_retention_for_every_pair_of_subsets() {
            let places = [
                place(2, 1, 8),
                place(1, 1, 4),
                place(1, 2, 4),
                place(1, 1, 8),
            ];
            for first_subset in 0..(1 << places.len()) {
                for second_subset in 0..(1 << places.len()) {
                    let mut exclusions = CaseExclusions::default();
                    let mut expected = places.to_vec();
                    for place in &places {
                        exclusions.insert(place);
                    }
                    for subset in [first_subset, second_subset] {
                        let before = expected.clone();
                        let mut observed = Vec::new();
                        let keep = |place: &CanonicalPlace| {
                            places.iter().enumerate().any(|(position, candidate)| {
                                candidate == place && subset & (1 << position) != 0
                            })
                        };
                        exclusions.retain(|place| {
                            observed.push(place.clone());
                            keep(place)
                        });
                        expected.retain(keep);
                        assert_eq!(observed, before);
                        assert_eq!(exclusions.iter().cloned().collect::<Vec<_>>(), expected);
                        for place in &places {
                            assert_eq!(exclusions.contains(place), expected.contains(place));
                        }
                    }
                    for place in places.iter().rev() {
                        exclusions.insert(place);
                        if !expected.contains(place) {
                            expected.push(place.clone());
                        }
                    }
                    assert_eq!(exclusions.iter().cloned().collect::<Vec<_>>(), expected);
                    for place in &places {
                        assert!(exclusions.contains(place));
                    }
                }
            }
        }
    }
}
use super::construction_frontier::ConstructionFrontier;
use super::construction_frontier::requires_owned_establishment;
use super::construction_frontier::transparent_storage;
use super::construction_frontier::zero_image_inactive_case;
use super::linear_obligations::LinearPlace;
use super::selected_evaluation::ExpressionRoutes;
use super::selected_evaluation::SelectedEvaluation;
use crate::flow::canonical_place_segments_may_overlap;
use case_exclusions::CaseExclusions;
pub(in crate::checks::multiplicity) use case_exclusions::{
    contains_observed_case, lies_under_observed_case,
};

use checked_trees::{
    ContextualCleanupDisposition, ContextualCleanupExcludedRegion as ExcludedRegion,
    ContextualCleanupExclusionCause as ExclusionCause, ContextualCleanupFacts as CleanupFacts,
    FlowClaimJoinExit, FlowPermissionEventFact,
};
use facts::{Place, PlaceRoot, PlaceSegment};
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionEventKind, PermissionEventSource,
};

#[derive(Clone, Default)]
pub(crate) struct RemainingCustody {
    /*
        Moved regions and conditional claim regions that were never established
        both lie outside the owner's cleanup complement.
    */
    excluded: Vec<ExcludedStorage>,
    /*
        Real transfers survive only where each predecessor transfers the region
        or proves its case absent. This is independent of possible exclusions.
    */
    definite_excluded: Vec<validation::CanonicalPlace>,
    /*
        Tag observations are facts, not transfers. They intersect at joins and
        are invalidated by writes; unioning them can erase every live alternative.
    */
    inactive_cases: CaseExclusions,
    storage_frontiers: Vec<(PlaceRoot, ConstructionFrontier)>,
    /*
        Linear locals whose whole storage is currently a zero image: present
        zero-filled bits that established no value and owe no obligation
        (ownership.md, "Establishment and erasure"). The first assignment
        retires the image as a hook-free whole discard.
    */
    zero_images: Vec<PlaceRoot>,
}

/*
    Cleanup normalizes absent regions by containment. Retain the actual
    consumed subregion separately as diagnostic provenance: coalescing an
    unestablished ancestor must not claim that ancestor itself was transferred.
*/
#[derive(Clone)]
struct ExcludedStorage {
    root: PlaceRoot,
    path: Vec<PlaceSegment>,
    consumed_path: Option<Vec<PlaceSegment>>,
    cause: ExclusionCause,
}

/*
    A receiving write's storage extent, paired with its first claim event in
    the current evaluation. Claim events name leaves, not the containing write.
*/
pub(super) struct StorageEstablishment {
    pub(super) event_ordinal: usize,
    pub(super) destination: validation::CanonicalPlace,
    /* All incoming paths transferred this value before replacement disposal. */
    pub(super) prior_value_transferred: bool,
}

/* A source evaluation position, distinct even when it emits no claim event. */
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CustodyCut {
    pub(crate) occurrence_ordinal: usize,
    pub(crate) permission_prefix: usize,
}

/*
    A completed invocation's physical write frontier at its exact event cut.
    Unknown writes detach historical source associations; they never grant
    replacement or transfer authority.
*/
pub(super) struct StorageBindingChange {
    pub(super) cut: CustodyCut,
    pub(super) source: PermissionEventSource,
    pub(super) evaluation: checked_trees::FlowPermissionEvaluation,
    pub(super) route: Vec<crate::checks::ownership_occurrences::EvaluationBranch>,
    pub(super) writes: Option<Vec<validation::CanonicalPlace>>,
}

/*
    A selected move's exact pre-transfer custody, captured by the producer.
    Permission events alone omit call invalidations and storage-only changes.
*/
pub(crate) struct TransferCustody {
    pub(crate) occurrence_ordinal: usize,
    pub(crate) events: std::ops::Range<usize>,
    pub(crate) places: Vec<LinearPlace>,
    pub(crate) remaining: RemainingCustody,
}

impl RemainingCustody {
    /*
        Compare the same occurrence after its exact receipt transfers. Holes
        are a set; construction order and case evidence remain exact.
    */
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
                .definite_excluded
                .iter()
                .filter(|place| place.root == root)
                .all(|place| other.definite_excluded.contains(place))
            && other
                .definite_excluded
                .iter()
                .filter(|place| place.root == root)
                .all(|place| self.definite_excluded.contains(place))
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

    /* A fresh construction vacancy is distinct from previously moved storage. */
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
                    && (lies_under_observed_case(&removed.segments, path)
                        || contains_observed_case(path, &removed.segments))
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
                    ConstructionFrontier::completed(program, parameter.type_reference),
                ));
            }
        }
        custody
    }

    /*
        Ordinary post-selection availability is the intersection. Selected
        cleanup retains the exact skipped-path frontier separately; this join
        never turns possible custody into unconditional disposal authority.
    */
    pub(super) fn join(&mut self, sibling: &Self) {
        let mut definite = self.definite_excluded.clone();
        for place in &sibling.definite_excluded {
            if !definite.contains(place) {
                definite.push(place.clone());
            }
        }
        definite.retain(|place| {
            self.definitely_absent(place.root, &place.segments)
                && sibling.definitely_absent(place.root, &place.segments)
        });
        let common_unestablished = sibling
            .excluded
            .iter()
            .map(|right| {
                right.cause == ExclusionCause::Unestablished
                    && self.excluded.iter().any(|left| {
                        left.root == right.root
                            && left.path == right.path
                            && left.cause == ExclusionCause::Unestablished
                    })
            })
            .collect::<Vec<_>>();
        for left in &mut self.excluded {
            if left.cause == ExclusionCause::Unestablished
                && !sibling.excluded.iter().any(|right| {
                    right.root == left.root
                        && right.path == left.path
                        && right.cause == ExclusionCause::Unestablished
                })
            {
                left.cause = ExclusionCause::Unknown;
            }
        }
        for (removed, common) in sibling.excluded.iter().zip(common_unestablished) {
            let cause = if removed.cause == ExclusionCause::Unestablished && !common {
                ExclusionCause::Unknown
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
        self.definite_excluded = definite;
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

    /* Reserve a deferred owned local without inventing a valid whole value. */
    pub(super) fn reserve(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        statement: &typed_trees::statement::StatementNode,
    ) {
        let Some(local) = statement.as_local_data() else {
            return;
        };
        if local.initial_value.is_valid()
            || !transparent_storage(program, local.type_reference)
            || !requires_owned_establishment(program, local.type_reference)
        {
            return;
        }
        self.storage_frontiers.push((
            PlaceRoot::Symbol(local.symbol),
            ConstructionFrontier::new(program, local.type_reference),
        ));
    }

    pub(super) fn has_storage_frontier(&self, root: PlaceRoot) -> bool {
        self.storage_frontiers
            .iter()
            .any(|(candidate, _)| *candidate == root)
    }

    /*
        Whether the reserved root is a whole value once `later` establishments
        join this custody's frontier: the retained path of a selected transfer
        sees both the pre-transfer frontier and what was stored afterwards.
        A root that is not under construction is already whole.
    */
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

    /*
        The authenticated storage root's established subtrees in establishment
        order, or none when no formation frontier is retained.
    */
    pub(super) fn established_paths(&self, root: PlaceRoot) -> &[Vec<PlaceSegment>] {
        self.storage_frontiers
            .iter()
            .find(|(candidate, _)| *candidate == root)
            .map_or(&[], |(_, frontier)| frontier.established.as_slice())
    }

    /*
        The path crosses a case proven inactive at this cut. Its payload is
        absent by tag evidence, not by a missing establishment.
    */
    pub(super) fn crosses_inactive_case(&self, root: PlaceRoot, path: &[PlaceSegment]) -> bool {
        self.inactive_cases.contains_prefix_of(root, path)
    }

    pub(super) fn is_present(&self, root: PlaceRoot, path: &[PlaceSegment]) -> bool {
        !self
            .excluded
            .iter()
            .any(|removed| removed.root == root && path.starts_with(&removed.path))
            && !self.inactive_cases.contains_prefix_of(root, path)
            && self
                .storage_frontiers
                .iter()
                .find(|(candidate, _)| *candidate == root)
                .is_none_or(|(_, frontier)| frontier.contains(path))
    }

    /*
        An allocated, still-owned home must retire even when no semantic
        cleanup-bearing subtree remains. Initialized unrestricted fields do
        not create disposal work. This is exit geometry, not a persistent removal
        that could prevent a later initializing write.
    */
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

    /*
        Actual spatial exclusions, without the synthetic exit-only retirement
        marker for a reserved home whose established children need no cleanup.
    */
    pub(super) fn excluded_paths(&self, root: PlaceRoot) -> impl Iterator<Item = &[PlaceSegment]> {
        self.excluded
            .iter()
            .filter(move |entry| entry.root == root)
            .map(|entry| entry.path.as_slice())
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
    ) -> Vec<(&'a [PlaceSegment], ExclusionCause)> {
        if path.is_empty() && self.needs_empty_retirement(program, root) {
            return vec![(&[], ExclusionCause::Unknown)];
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
            .filter(|inactive| {
                inactive.root == root && contains_observed_case(path, &inactive.segments)
            })
            .map(|inactive| &inactive.segments[path.len()..])
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

    /*
        `observe` for an ordinary statement edge, where every root dies in
        its declared order.
    */
    #[allow(clippy::too_many_arguments)]
    pub(super) fn observe_ordinary(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        events: &[FlowPermissionEventFact],
        segments: &arena::PagedArenaSet<PlaceSegment>,
        output: &mut CleanupFacts,
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

    /*
        Replay the statement from its entry custody, joining expression paths
        before its receiving write. A conditional restoration cannot erase a
        sibling's removal merely because its event appears later in the roster.
    */
    pub(super) fn observe_evaluation(
        &mut self,
        program: &dyn validation::CheckingInput,
        routes: &ExpressionRoutes,
        calls: &[checked_trees::FlowCallFact],
        events: &[FlowPermissionEventFact],
        segments: &arena::PagedArenaSet<PlaceSegment>,
        output: &mut CleanupFacts,
        establishments: &[StorageEstablishment],
    ) -> Result<(), diagnostics::Diagnostic> {
        if events.is_empty() {
            return Ok(());
        }
        let mut selected = SelectedEvaluation::new();
        let join = |_branch, current: &mut Self, sibling: &Self| {
            current.join(sibling);
            Ok(())
        };
        let begin = |branch, current: &mut Self| {
            routes.observe_owned_case(branch, current);
            Ok(())
        };
        for (ordinal, event) in events.iter().enumerate() {
            selected.enter_with(routes.event(event, calls), self, begin, |_, _| Ok(()), join)?;
            let receiving = establishments
                .iter()
                .filter(|establishment| establishment.event_ordinal == ordinal)
                .map(|establishment| StorageEstablishment {
                    event_ordinal: 0,
                    destination: establishment.destination.clone(),
                    prior_value_transferred: establishment.prior_value_transferred,
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
        selected.enter_with(&[], self, begin, |_, _| Ok(()), join)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn observe(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        events: &[FlowPermissionEventFact],
        segments: &arena::PagedArenaSet<PlaceSegment>,
        output: &mut CleanupFacts,
        exit: FlowClaimJoinExit,
        establishments: &[StorageEstablishment],
        selected: bool,
        establishment_ordered_roots: &[PlaceRoot],
    ) {
        for (event_ordinal, event) in events.iter().enumerate() {
            if event.access != PermissionAccess::Owned {
                continue;
            }
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
                        prior_value_transferred: establishment.prior_value_transferred,
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
                let zero_image = path.is_empty() && self.zero_images.contains(&event.root);
                let mut exclusions = if zero_image {
                    Vec::new()
                } else {
                    self.excluded
                        .iter()
                        .filter(|removed| removed.root == event.root)
                        .map(|removed| ExcludedRegion {
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
                    exclusions.push(ExcludedRegion {
                        root: event.root,
                        segments: Default::default(),
                        cause: ExclusionCause::Unknown,
                    });
                }
                let establishment_ordered = establishment_ordered_roots.contains(&event.root);
                let inactive_cases = self
                    .inactive_cases
                    .iter()
                    .filter(|inactive| inactive.root == event.root)
                    .map(|inactive| ExcludedRegion {
                        root: inactive.root,
                        cause: ExclusionCause::InactiveCase,
                        segments: output
                            .segments
                            .insert_many(inactive.segments.iter().copied()),
                    })
                    .collect::<Vec<_>>();
                let definite_exclusions = exclusions
                    .iter()
                    .filter(|excluded| {
                        self.definitely_transferred(
                            excluded.root,
                            output.segments.span_or_empty(excluded.segments),
                        )
                    })
                    .cloned()
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
                    definite_exclusions: output.exclusions.insert_many(definite_exclusions),
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
                if event.kind == PermissionEventKind::Establish
                    && event.multiplicity == Multiplicity::Linear
                    && !event.obligation_live
                    && let PermissionEventSource::Statement { statement_index } = event.source
                    && let Some(state) = program.state_by_symbol(event.state_symbol)
                    && let Some(local) =
                        crate::flow::state_statements(program, state.statement_nodes)
                            .get(statement_index)
                            .and_then(|statement| statement.as_local_data())
                    && event.root == PlaceRoot::Symbol(local.symbol)
                    && !local.initial_value.is_valid()
                    && !validation::type_requires_establishment(program, local.type_reference)
                    && let Some(inactive) =
                        zero_image_inactive_case(program, local.type_reference, path)
                {
                    self.exclude_absent_case(&validation::CanonicalPlace {
                        root: event.root,
                        segments: inactive.to_vec(),
                    });
                    return;
                }
                self.invalidate_case_observations(program, event.root, path);
                self.excluded.retain(|removed| {
                    removed.root != event.root || !removed.path.starts_with(path)
                });
                self.definite_excluded.retain(|removed| {
                    removed.root != event.root || !removed.segments.starts_with(path)
                });
                if event.multiplicity == Multiplicity::Linear && !event.obligation_live {
                    if path.is_empty() {
                        crate::shared::collections::push_unique(&mut self.zero_images, event.root);
                    } else {
                        let cause = if event.kind == PermissionEventKind::Establish {
                            ExclusionCause::Unestablished
                        } else {
                            ExclusionCause::Unknown
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
                self.transfer_storage(event.root, path);
            }
        }
    }

    /*
        The source schedule proved this whole case absent on this alternative.
        Preserve the containing home; this is not an owned transfer or death.
    */
    pub(super) fn exclude_absent_case(&mut self, place: &validation::CanonicalPlace) {
        debug_assert!(matches!(
            place.segments.last(),
            Some(PlaceSegment::Case { .. })
        ));
        self.inactive_cases.insert(place);
    }

    fn definitely_absent(&self, root: PlaceRoot, path: &[PlaceSegment]) -> bool {
        self.definitely_transferred(root, path)
            || self.inactive_cases.contains_prefix_of(root, path)
    }

    pub(super) fn definitely_transferred(&self, root: PlaceRoot, path: &[PlaceSegment]) -> bool {
        self.definite_excluded
            .iter()
            .any(|place| place.root == root && path.starts_with(&place.segments))
    }

    /*
        A checked case projection authenticates the active tag at this actual
        transfer. Nested claim events cannot select a case their whole source
        expression merely contains. Borrowed routes confer no owned cleanup.
    */
    pub(super) fn observe_selected_transfer(
        &mut self,
        program: &dyn validation::CheckingInput,
        state: symbols::SymbolHandle,
        statement: usize,
        root: PlaceRoot,
        expression: typed_trees::expression::ExpressionHandle,
        event_path: &[PlaceSegment],
    ) {
        let Some(place) = validation::ownership_place_from_expression_in_state(
            program, state, statement, expression,
        ) else {
            return;
        };
        if place.root != root || !event_path.starts_with(&place.segments) {
            return;
        }
        for (position, segment) in place.segments.iter().enumerate() {
            if let PlaceSegment::Case { variant } = segment {
                self.select_owned_case(
                    program,
                    state,
                    statement,
                    &validation::CanonicalPlace {
                        root: place.root,
                        segments: place.segments[..position].to_vec(),
                    },
                    *variant,
                    true,
                );
            }
        }
    }

    /*
        Tag observations retain only exact owned storage. Both guard selection
        and selected value transfers use this same declaration/authority walk.
    */
    pub(super) fn select_owned_case(
        &mut self,
        program: &dyn validation::CheckingInput,
        state: symbols::SymbolHandle,
        statement: usize,
        place: &validation::CanonicalPlace,
        variant: symbols::SymbolHandle,
        positive: bool,
    ) {
        let Some(paths) = Self::owned_case_paths(program, state, statement, place) else {
            return;
        };
        if !paths.iter().any(|(selected, _)| *selected == variant) {
            return;
        }
        for (selected, path) in paths {
            if (selected == variant) != positive {
                self.exclude_absent_case(&path);
            }
        }
    }

    /// Prepare the same owned-storage checks once for a saved branch
    /// decision. No path is inactive until that decision selects its outcome.
    pub(super) fn owned_case_paths(
        program: &dyn validation::CheckingInput,
        state: symbols::SymbolHandle,
        statement: usize,
        place: &validation::CanonicalPlace,
    ) -> Option<Vec<(symbols::SymbolHandle, validation::CanonicalPlace)>> {
        let machine = program.machine_holding_state(state)?;
        let state_definition = program.state_in_machine(machine, state)?;
        if !crate::flow::stable_case_subject(program, machine, state_definition, statement, place)
            || !(0..=place.segments.len()).all(|length| {
                validation::canonical_place_type_reference(
                    program,
                    state,
                    statement,
                    &validation::CanonicalPlace {
                        root: place.root,
                        segments: place.segments[..length].to_vec(),
                    },
                )
                .is_some_and(|reference| validation::reference_shell(program, reference).is_none())
            })
        {
            return None;
        }
        let data = validation::canonical_place_type_reference(program, state, statement, place)
            .and_then(|reference| program.closed_data_definition(reference))?;
        let variants = program
            .data_members(data)
            .iter()
            .filter_map(|member| match member {
                typed_trees::data::DataMember::Variant(row) => Some(row.symbol),
                _ => None,
            })
            .collect::<Vec<_>>();
        Some(
            variants
                .into_iter()
                .map(|variant| {
                    let mut path = place.clone();
                    path.segments.push(PlaceSegment::Case { variant });
                    (variant, path)
                })
                .collect(),
        )
    }

    fn invalidate_case_observations(
        &mut self,
        program: &dyn validation::CheckingInput,
        root: PlaceRoot,
        path: &[PlaceSegment],
    ) {
        for removed in &mut self.excluded {
            if removed.root == root
                && removed.cause == ExclusionCause::Unestablished
                && canonical_place_segments_may_overlap(program, &removed.path, path)
            {
                removed.cause = ExclusionCause::Unknown;
            }
        }
        self.inactive_cases.retain(|inactive| {
            inactive.root != root
                || !canonical_place_segments_may_overlap(program, &inactive.segments, path)
        });
    }

    /*
        Forget observations at the exact returned invocation before any owner
        cleanup. Borrowed writes can change tags without restoring a linear
        claim; unknown effects forget facts but never manufacture authority.
    */
    pub(super) fn invalidate_call_observations(
        &mut self,
        program: &dyn validation::CheckingInput,
        writes: Option<&[validation::CanonicalPlace]>,
    ) {
        let Some(writes) = writes else {
            self.inactive_cases.clear();
            for removed in &mut self.excluded {
                if removed.cause == ExclusionCause::Unestablished {
                    removed.cause = ExclusionCause::Unknown;
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
        cause: ExclusionCause,
        consumed_path: Option<&[PlaceSegment]>,
    ) {
        if let Some(prior) = self
            .excluded
            .iter_mut()
            .find(|prior| prior.root == root && path.starts_with(&prior.path))
        {
            if prior.path != path || prior.cause != cause {
                prior.cause = ExclusionCause::Unknown;
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
            ExclusionCause::Unknown
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

    /*
        Only a real transfer/disposal cause diagnoses consumption. A missing
        establishment or inactive tag alone remains a different source failure.
    */
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

    /*
        A captured owned move transfers its selected storage along with the
        claim frontier. Descendant claim events alone cannot record a whole
        carrier transfer: retaining that home would dispose it on the edge.
    */
    pub(super) fn transfer_storage(&mut self, root: PlaceRoot, path: &[PlaceSegment]) {
        self.remove(root, path, ExclusionCause::Consumed, Some(path));
        if !self.definitely_transferred(root, path) {
            self.definite_excluded
                .retain(|place| place.root != root || !place.segments.starts_with(path));
            self.definite_excluded.push(validation::CanonicalPlace {
                root,
                segments: path.to_vec(),
            });
        }
    }

    /*
        A checked normal-continuation disposition retires the containing home,
        even when its permission receipts name only contained claims.
    */
    pub(super) fn retire_storage(
        &mut self,
        root: PlaceRoot,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        source: PermissionEventSource,
        output: &mut CleanupFacts,
    ) {
        if !output.dispositions.iter().any(|(_, row)| {
            row.machine == machine && row.state == state && row.source == source && row.root == root
        }) {
            let exclusions = output.exclusions.insert_many([ExcludedRegion {
                root,
                segments: Default::default(),
                cause: ExclusionCause::Consumed,
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
                definite_exclusions: Default::default(),
                inactive_cases: Default::default(),
                establishment_ordered: false,
                zero_image: false,
            });
        }
        self.remove(root, &[], ExclusionCause::Consumed, Some(&[]));
    }

    /*
        Claim events name leaves; the successful write also establishes its
        containing storage region. A child write never clears a removed parent.
    */
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
        self.definite_excluded
            .retain(|removed| removed.root != root || !removed.segments.starts_with(path));
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
                ConstructionFrontier::completed(program, local.type_reference),
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
