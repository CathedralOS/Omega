use crate::source_state::AuthoredDeclarationSelectionOccurrenceId;

#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub struct AuthoredSelectionExclusions {
    occurrences: Box<[AuthoredDeclarationSelectionOccurrenceId]>,
}

impl From<Vec<AuthoredDeclarationSelectionOccurrenceId>> for AuthoredSelectionExclusions {
    fn from(mut occurrences: Vec<AuthoredDeclarationSelectionOccurrenceId>) -> Self {
        occurrences.sort_unstable();
        occurrences.dedup();
        Self {
            occurrences: occurrences.into_boxed_slice(),
        }
    }
}

impl AuthoredSelectionExclusions {
    pub fn contains(&self, occurrence: AuthoredDeclarationSelectionOccurrenceId) -> bool {
        self.occurrences.binary_search(&occurrence).is_ok()
    }

    pub fn into_occurrences(self) -> Vec<AuthoredDeclarationSelectionOccurrenceId> {
        self.occurrences.into_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::AuthoredSelectionExclusions;
    use crate::source_state::{
        AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
        AuthoredDeclarationSelections,
    };
    use source_text::SourceSpan;
    use symbols::SymbolHandle;

    #[test]
    fn exclusion_membership_follows_normalized_occurrences() {
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
        let mut exclusions = AuthoredSelectionExclusions::from(vec![
            occurrences[2],
            occurrences[0],
            occurrences[2],
        ]);
        let expected = [occurrences[0], occurrences[2]];
        assert_eq!(exclusions.occurrences.as_ref(), expected.as_slice());
        assert!(exclusions.contains(occurrences[0]));
        assert!(!exclusions.contains(occurrences[1]));
        assert_eq!(exclusions.clone(), exclusions);

        let mut replacement = std::mem::take(&mut exclusions).into_occurrences();
        assert!(exclusions.occurrences.is_empty());
        assert!(!exclusions.contains(occurrences[0]));
        replacement[1] = occurrences[3];
        exclusions = replacement.into();
        assert!(exclusions.contains(occurrences[3]));
        assert!(!exclusions.contains(occurrences[2]));
        assert_eq!(
            exclusions.occurrences.as_ref(),
            &[occurrences[0], occurrences[3]]
        );
        assert_eq!(
            AuthoredSelectionExclusions::from(Vec::new()),
            AuthoredSelectionExclusions::default(),
        );
    }
}
