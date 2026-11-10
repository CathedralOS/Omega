/*
May-sets: what a reference value may denote and which machine a callable
value may select. A value never changes, so its may-set is fixed; block
parameters receive the union over their arrivals. Unknown is a member of the
vocabulary, and it means everything.
*/

use semantic_vocabulary::{MachineId, ValueId};

use crate::ExactPlaceId;

/// One value's may-set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaySetRow {
    pub value: ValueId,
    pub denotes: Denotes,
}

/// The two things a may-set ranges over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Denotes {
    /// The places a reference may denote. `wiki/spec/language/lifetimes.md`:
    /// "an unresolved dynamic selection conservatively retains all possible
    /// sources." A member place stands for itself and everything beneath it;
    /// `wiki/spec/terminal-psi/ownership.md`: "an unresolved projection
    /// conservatively pins its whole spelled root."
    Places(MaySet<ExactPlaceId>),
    /// The machines a callable value may select. `wiki/spec/language/effects.md`,
    /// "Known dynamic selections": "Joins retain all possible demands and only
    /// guarantees justified for every possible selection; an unknown
    /// alternative contributes the requirement ceiling." So `Unknown` here is
    /// answered by the callable type's published contract, never by a body.
    Machines(MaySet<MachineId>),
}

/// A finite set of candidates, or every candidate.
///
/// Invariant: `Members` is sorted and has no duplicates; build it with
/// `MaySet::of`. An empty `Members` is a proved empty set, not an unknown one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaySet<T> {
    Members(Vec<T>),
    /// Nothing is known, so every candidate is possible.
    Unknown,
}

impl<T: Ord> MaySet<T> {
    pub fn of(members: impl IntoIterator<Item = T>) -> Self {
        let mut members: Vec<T> = members.into_iter().collect();
        members.sort();
        members.dedup();
        Self::Members(members)
    }

    /// The join at a reconvergence: union, and unknown absorbs.
    pub fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Members(mut left), Self::Members(right)) => {
                left.extend(right);
                Self::of(left)
            }
            _ => Self::Unknown,
        }
    }

    pub fn may_contain(&self, candidate: &T) -> bool {
        match self {
            // ponytail: linear, so an unsorted `Members` built by hand still
            // answers soundly; binary search once sets are measured large.
            Self::Members(members) => members.contains(candidate),
            Self::Unknown => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MaySet;

    #[test]
    fn join_is_union_and_unknown_absorbs() {
        let joined = MaySet::of([3, 1]).join(MaySet::of([2, 3]));
        assert_eq!(joined, MaySet::Members(vec![1, 2, 3]));
        assert_eq!(joined.clone().join(MaySet::Unknown), MaySet::Unknown);
        assert!(MaySet::<u8>::Unknown.may_contain(&9));
        assert!(!joined.may_contain(&9));
        assert!(!MaySet::<u8>::of([]).may_contain(&0));
    }
}
