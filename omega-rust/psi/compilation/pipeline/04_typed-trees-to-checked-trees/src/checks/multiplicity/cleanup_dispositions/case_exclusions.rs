//! Invocation-local case observations in first-observation order.
//!
//! Permission replay revisits the same authenticated exclusions at successive
//! cuts. Keep one exact-identity index alongside the publication order instead
//! of comparing every prior structural path on every visit. This index decides
//! only duplicate identity; it supplies no membership or overlap evidence.

use validation::CanonicalPlace;

mod lookup_profile {
    use std::sync::{
        OnceLock,
        atomic::{AtomicUsize, Ordering},
    };
    use validation::CanonicalPlace;

    static ENABLED: OnceLock<bool> = OnceLock::new();
    static LOOKUPS: AtomicUsize = AtomicUsize::new(0);
    static HITS: AtomicUsize = AtomicUsize::new(0);
    static COMPARISONS: AtomicUsize = AtomicUsize::new(0);
    static SAME_ROOT: AtomicUsize = AtomicUsize::new(0);
    static PREFIX_SEGMENTS: AtomicUsize = AtomicUsize::new(0);
    static EXACT: AtomicUsize = AtomicUsize::new(0);
    static SIZE_BUCKETS: [AtomicUsize; 16] = [const { AtomicUsize::new(0) }; 16];

    fn enabled() -> bool {
        *ENABLED.get_or_init(|| std::env::var_os("OMEGA_CASE_INDEX_PROFILE").is_some())
    }

    pub(super) fn comparison(left: &CanonicalPlace, right: &CanonicalPlace) {
        if !enabled() {
            return;
        }
        COMPARISONS.fetch_add(1, Ordering::Relaxed);
        if left.root == right.root {
            SAME_ROOT.fetch_add(1, Ordering::Relaxed);
            PREFIX_SEGMENTS.fetch_add(
                left.segments
                    .iter()
                    .zip(&right.segments)
                    .take_while(|(left, right)| left == right)
                    .count(),
                Ordering::Relaxed,
            );
        }
        EXACT.fetch_add(usize::from(left == right), Ordering::Relaxed);
    }

    pub(super) fn lookup(length: usize, found: bool) {
        if !enabled() {
            return;
        }
        let bucket = if length == 0 {
            0
        } else {
            (length.ilog2() as usize + 1).min(15)
        };
        SIZE_BUCKETS[bucket].fetch_add(1, Ordering::Relaxed);
        HITS.fetch_add(usize::from(found), Ordering::Relaxed);
        let count = LOOKUPS.fetch_add(1, Ordering::Relaxed) + 1;
        if count == 1 || count % 10000 == 0 {
            eprintln!(
                "case-index-profile lookups={} hits={} comparisons={} same_root={} prefix_segments={} exact={} size_log2={:?}",
                count,
                HITS.load(Ordering::Relaxed),
                COMPARISONS.load(Ordering::Relaxed),
                SAME_ROOT.load(Ordering::Relaxed),
                PREFIX_SEGMENTS.load(Ordering::Relaxed),
                EXACT.load(Ordering::Relaxed),
                SIZE_BUCKETS
                    .each_ref()
                    .map(|count| count.load(Ordering::Relaxed))
            );
        }
    }
}

#[derive(Clone, Default)]
pub(super) struct CaseExclusions {
    observations: Vec<CanonicalPlace>,
    by_identity: Vec<usize>,
}

impl CaseExclusions {
    pub(super) fn insert(&mut self, place: &CanonicalPlace) {
        let lookup = self.by_identity.binary_search_by(|&position| {
            lookup_profile::comparison(&self.observations[position], place);
            self.observations[position].compare_identity(place)
        });
        lookup_profile::lookup(self.by_identity.len(), lookup.is_ok());
        let Err(position) = lookup else {
            return;
        };
        self.by_identity.insert(position, self.observations.len());
        self.observations.push(place.clone());
    }

    pub(super) fn contains(&self, place: &CanonicalPlace) -> bool {
        let lookup = self.by_identity.binary_search_by(|&position| {
            lookup_profile::comparison(&self.observations[position], place);
            self.observations[position].compare_identity(place)
        });
        lookup_profile::lookup(self.by_identity.len(), lookup.is_ok());
        lookup.is_ok()
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &CanonicalPlace> {
        self.observations.iter()
    }

    pub(super) fn retain(&mut self, mut keep: impl FnMut(&CanonicalPlace) -> bool) {
        let previous_length = self.observations.len();
        let mut remapped_positions = Vec::new();
        let mut previous_position = 0;
        let mut retained_position = 0;
        self.observations.retain(|place| {
            let retained = keep(place);
            if !retained && remapped_positions.is_empty() {
                // Until the first removal every position stays unchanged.
                // A no-op invalidation needs neither scratch nor index work.
                remapped_positions.reserve(previous_length);
                remapped_positions.extend(0..previous_position);
            }
            if !retained || !remapped_positions.is_empty() {
                remapped_positions.push(if retained {
                    retained_position
                } else {
                    usize::MAX
                });
            }
            previous_position += 1;
            retained_position += usize::from(retained);
            retained
        });
        if self.observations.len() == previous_length {
            return;
        }
        // Identity keys do not change when their rows compact. Keep their
        // existing sorted order and translate positions instead of comparing
        // every surviving structural path again. Removed rows have no index.
        self.by_identity.retain_mut(|position| {
            let remapped = remapped_positions[*position];
            if remapped == usize::MAX {
                false
            } else {
                *position = remapped;
                true
            }
        });
    }

    pub(super) fn clear(&mut self) {
        self.observations.clear();
        self.by_identity.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::CaseExclusions;
    use facts::{PlaceRoot, PlaceSegment};
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
