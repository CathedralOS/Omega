//! Exact occurrence membership excluded by declaration-root projection.
//!
//! Projection establishes which occurrences leave; this carrier only freezes
//! that sorted membership and its structural-query fingerprint together. Typed
//! queries read the fingerprint frequently, while projection replaces the set
//! rarely. Computing the same fold at construction avoids scanning the entire
//! ledger subset on every query. Private storage prevents membership changes
//! from leaving the fingerprint stale; it is never selection authority.

use crate::AuthoredDeclarationSelectionOccurrenceId;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthoredSelectionExclusions {
    occurrences: Box<[AuthoredDeclarationSelectionOccurrenceId]>,
    fingerprint: u64,
}

impl From<Vec<AuthoredDeclarationSelectionOccurrenceId>> for AuthoredSelectionExclusions {
    fn from(mut occurrences: Vec<AuthoredDeclarationSelectionOccurrenceId>) -> Self {
        occurrences.sort_unstable();
        occurrences.dedup();
        let fingerprint = occurrences
            .iter()
            .fold(occurrences.len() as u64, |fingerprint, occurrence| {
                fingerprint.rotate_left(7) ^ occurrence.ordinal()
            });
        Self {
            occurrences: occurrences.into_boxed_slice(),
            fingerprint,
        }
    }
}

impl AuthoredSelectionExclusions {
    pub fn contains(&self, occurrence: AuthoredDeclarationSelectionOccurrenceId) -> bool {
        self.occurrences.binary_search(&occurrence).is_ok()
    }

    /// Transfer membership to projection scratch; publishing it reconstructs
    /// both sorted membership and its fingerprint through `From`.
    pub fn into_occurrences(self) -> Vec<AuthoredDeclarationSelectionOccurrenceId> {
        self.occurrences.into_vec()
    }

    pub(super) fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
}

#[cfg(test)]
mod tests {
    use super::AuthoredSelectionExclusions;
    use crate::{
        AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
        AuthoredDeclarationSelections,
    };
    use source::SourceSpan;
    use symbols::SymbolHandle;

    #[test]
    fn exclusion_membership_and_fingerprint_are_published_together() {
        let mut ledger = AuthoredDeclarationSelections::default();
        let occurrences = (0..4)
            .map(|_| {
                ledger
                    .record_resolved(
                        SourceSpan::default(),
                        AuthoredDeclarationSelectionExposure::PrivateImplementation,
                        AuthoredDeclarationSelectionKind::MemberAccess,
                        SymbolHandle::from_arena_index(1),
                    )
                    .expect("valid symbol and available ledger capacity")
            })
            .collect::<Vec<_>>();
        let mut exclusions =
            AuthoredSelectionExclusions::from(vec![occurrences[2], occurrences[0], occurrences[2]]);
        let expected = [occurrences[0], occurrences[2]];
        let expected_fingerprint = expected
            .iter()
            .fold(expected.len() as u64, |fingerprint, occurrence| {
                fingerprint.rotate_left(7) ^ occurrence.ordinal()
            });
        assert_eq!(exclusions.occurrences.as_ref(), expected.as_slice());
        assert_eq!(exclusions.fingerprint(), expected_fingerprint);
        assert!(exclusions.contains(occurrences[0]));
        assert!(!exclusions.contains(occurrences[1]));
        assert_eq!(exclusions.clone(), exclusions);

        let mut replacement = std::mem::take(&mut exclusions).into_occurrences();
        assert_eq!(exclusions.fingerprint(), 0);
        assert!(!exclusions.contains(occurrences[0]));
        replacement[1] = occurrences[3];
        exclusions = replacement.into();
        assert!(exclusions.contains(occurrences[3]));
        assert!(!exclusions.contains(occurrences[2]));
        assert_ne!(exclusions.fingerprint(), expected_fingerprint);
        assert_eq!(
            AuthoredSelectionExclusions::from(Vec::new()),
            AuthoredSelectionExclusions::default(),
        );
    }
}
