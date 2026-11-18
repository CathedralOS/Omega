//! Canonical finite sets over ordered caller-owned identities.
use std::collections::BTreeSet;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedIdentitySet<Id>(BTreeSet<Id>);

impl<Id> Default for NormalizedIdentitySet<Id> {
    fn default() -> Self {
        Self(BTreeSet::new())
    }
}

impl<Id: Copy + Ord> NormalizedIdentitySet<Id> {
    pub const fn none() -> Self {
        Self(BTreeSet::new())
    }

    pub fn from_normalized_identities(identities: impl IntoIterator<Item = Id>) -> Self {
        Self(identities.into_iter().collect())
    }

    pub fn union(&self, other: &Self) -> Self {
        Self(self.0.union(&other.0).copied().collect())
    }

    pub fn contains(&self, required: &Self) -> bool {
        required.0.is_subset(&self.0)
    }

    pub fn identities(&self) -> impl Iterator<Item = Id> + '_ {
        self.0.iter().copied()
    }
}
