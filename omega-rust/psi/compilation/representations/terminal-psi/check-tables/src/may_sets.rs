/*
May-sets: what a reference may denote and which machine a callable may select.
They join by union, and unknown means everything.

The two are keyed on different things because the lowered form represents
them differently:

- A reference is a structural place, not a value. Terminal foundation
  validation requires it: "reference establishment requires a structural
  result" whose type shape is `StructuralTypeShape::Reference`
  (`terminal-psi/codec/src/foundation_validation.rs:7306-7310`). A reference
  place can be rewritten, so its referent set belongs to the write that
  installed the reference: `bindings::Content::Reference`.
- A callable selector is a scalar value: `SelectImplementation { table }`
  produces a scalar result that `CallDynamic { selector, .. }` consumes
  (`terminal-psi/semantics/src/descriptors.rs:12795-12803` reads it through
  `result.scalar_ref()`). A value never changes, so its selection set is a
  column of its row: `values::ValueRow::selects`.
*/

use semantic_vocabulary::MachineId;

use crate::ExactPlaceId;

/// The places one installed reference may denote.
/// `wiki/spec/language/lifetimes.md`: "an unresolved dynamic selection
/// conservatively retains all possible sources." A member place stands for
/// itself and everything beneath it; `wiki/spec/terminal-psi/ownership.md`:
/// "an unresolved projection conservatively pins its whole spelled root."
pub type ReferentSet = MaySet<ExactPlaceId>;

/// The machines one callable selector value may select.
/// `wiki/spec/language/effects.md`, "Known dynamic selections": "Joins retain
/// all possible demands and only guarantees justified for every possible
/// selection; an unknown alternative contributes the requirement ceiling." So
/// `Unknown` here is answered by the callable type's published contract,
/// never by a body.
pub type SelectionSet = MaySet<MachineId>;

/// A finite set of candidates, or every candidate.
///
/// Build `Members` with `MaySet::of`, which sorts and removes duplicates. An
/// empty `Members` is a proved empty set, not an unknown one.
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
