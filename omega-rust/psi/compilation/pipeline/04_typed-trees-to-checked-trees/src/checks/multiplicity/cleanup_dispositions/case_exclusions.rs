//! Invocation-local case observations in first-observation order.
//!
//! Permission replay forks the same authenticated exclusions at successive
//! cuts. Share the immutable table and paths until a branch changes its facts;
//! copying every structural path at each cut makes large checking bodies retain
//! the same evidence many times. The table keeps its publication order and exact
//! identity index together. Hashes only filter candidates: full structural
//! equality decides collisions, and invalidation still visits every original
//! observation exactly once in order.

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
        // A collision keeps the first indexed row, while complete retained
        // paths distinguish every other observation with that fingerprint.
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
        table.observations.push(CaseObservation {
            place: Arc::new(place.clone()),
            fingerprint,
        });
    }

    pub(super) fn contains(&self, place: &CanonicalPlace) -> bool {
        self.identity_position(Self::observation_fingerprint(place), place)
            .is_some()
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &CanonicalPlace> {
        self.table
            .observations
            .iter()
            .map(|observation| observation.place.as_ref())
    }

    pub(super) fn retain(&mut self, mut keep: impl FnMut(&CanonicalPlace) -> bool) {
        // An unrelated write must not copy a shared table. Find the first
        // removal before detaching, retaining the predicate's original order
        // and exactly-once evaluation even when it carries observation state.
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
        // The first removed row and its prefix were already tested. Rebuild
        // only after real removal, pointing each fingerprint at its first
        // surviving observation without rehashing or changing publication order.
        table.by_identity.clear();
        for (position, observation) in table.observations.iter().enumerate() {
            table
                .by_identity
                .entry(observation.fingerprint)
                .or_insert(position);
        }
    }

    pub(super) fn clear(&mut self) {
        // Forget this branch's facts without detaching and copying its sibling.
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

    fn place(root_generation: u32, case_generation: u32, element_index: usize) -> CanonicalPlace {
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
    fn repeated_cuts_preserve_first_observation_and_complete_identities() {
        let places = [
            place(2, 1, 8),
            place(1, 1, 4),
            place(1, 2, 4),
            place(1, 1, 8),
        ];
        // The old growing scan is the oracle, including duplicates interleaved
        // with different roots, generations and structural coordinates.
        for length in 0..=5 {
            for sequence in 0..places.len().pow(length) {
                let mut digits = sequence;
                let mut exclusions = CaseExclusions::default();
                let mut expected = Vec::new();
                for _ in 0..length {
                    let place = &places[digits % places.len()];
                    digits /= places.len();
                    exclusions.insert(place);
                    if !expected.contains(place) {
                        expected.push(place.clone());
                    }
                }
                assert_eq!(exclusions.iter().cloned().collect::<Vec<_>>(), expected);
                for place in &places {
                    assert_eq!(exclusions.contains(place), expected.contains(place));
                }
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
        // A sibling's opposite publication order must not reorder the join.
        for place in [&places[3], &places[0], &places[2]] {
            sibling.insert(place);
        }
        exclusions.retain(|place| sibling.contains(place));
        assert_eq!(
            exclusions.iter().collect::<Vec<_>>(),
            vec![&places[0], &places[2], &places[3]]
        );
        // A receiving write invalidates its selected observations. Repeating
        // an unrelated cut retains the index; a later establishment appends.
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
                    // Invalidation may be stateful: each original row is
                    // observed exactly once, in publication order.
                    assert_eq!(observed, before);
                    assert_eq!(exclusions.iter().cloned().collect::<Vec<_>>(), expected);
                    for place in &places {
                        assert_eq!(exclusions.contains(place), expected.contains(place));
                    }
                }
                // Reinsertion exercises the translated index after two
                // compactions, including first/last/all/no removals.
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
