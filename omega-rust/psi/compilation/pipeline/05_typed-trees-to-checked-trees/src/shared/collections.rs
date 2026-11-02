//! Ordered rosters: one owner for first-occurrence accumulation.
//!
//! Checked-tree builders accumulate ordered rosters — dependencies, origins,
//! identities, dispositions, claims. The policy is order-preserving
//! uniqueness: the first occurrence of a value fixes its position and every
//! later occurrence is dropped, so a roster reads in the order the producing
//! walk met its entries.
//!
//! Keeping that policy here lets a producer state what it accumulates instead
//! of restating how uniqueness is decided. The per-site rescan it replaces
//! (`if !roster.contains(&entry) { roster.push(entry); }`, its `fold`
//! spelling, and the one-off `push_unique_*` wrappers) let one accumulator
//! drift into a different rule — a `clone` on the insert path, a skipped
//! validity check, a set that reordered the roster — without the rosters that
//! consume it noticing.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

/// Append `value` unless an equal value is already present, keeping the
/// first occurrence's position.
pub(crate) fn push_unique<T: PartialEq>(values: &mut Vec<T>, value: T) {
    if !values.contains(&value) {
        values.push(value);
    }
}

/// Collect the first occurrence of each value, in iteration order.
pub(crate) fn unique_in_order<T: PartialEq>(values: impl IntoIterator<Item = T>) -> Vec<T> {
    let mut unique = Vec::new();
    for value in values {
        push_unique(&mut unique, value);
    }
    unique
}

/// The same first-occurrence roster for one that grows past a few dozen
/// entries, where rescanning for each append makes accumulation quadratic.
/// Equality still decides uniqueness; the hash only chooses which earlier
/// entries to compare, and no value is cloned.
pub(crate) struct UniqueRoster<T> {
    values: Vec<T>,
    positions_by_hash: HashMap<u64, Vec<usize>>,
}

impl<T> Default for UniqueRoster<T> {
    fn default() -> Self {
        Self {
            values: Vec::new(),
            positions_by_hash: HashMap::new(),
        }
    }
}

impl<T: Hash + Eq> UniqueRoster<T> {
    pub(crate) fn push(&mut self, value: T) {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        let positions = self.positions_by_hash.entry(hasher.finish()).or_default();
        if positions
            .iter()
            .any(|position| self.values[*position] == value)
        {
            return;
        }
        positions.push(self.values.len());
        self.values.push(value);
    }

    pub(crate) fn as_slice(&self) -> &[T] {
        &self.values
    }

    pub(crate) fn into_vec(self) -> Vec<T> {
        self.values
    }
}

#[cfg(test)]
mod tests {
    use super::{UniqueRoster, push_unique, unique_in_order};
    use std::hash::{Hash, Hasher};

    #[test]
    fn push_unique_keeps_first_occurrence_order() {
        let mut values = vec![3, 1];
        push_unique(&mut values, 1);
        push_unique(&mut values, 7);
        push_unique(&mut values, 3);
        assert_eq!(values, vec![3, 1, 7]);
    }

    #[test]
    fn unique_in_order_drops_repeats() {
        assert_eq!(unique_in_order([2, 1, 2, 2, 3, 1]), vec![2, 1, 3]);
        assert!(unique_in_order(Vec::<u8>::new()).is_empty());
    }

    // Every value hashes alike, so uniqueness must come from equality.
    #[derive(Debug, PartialEq, Eq)]
    struct Colliding(u32);

    impl Hash for Colliding {
        fn hash<H: Hasher>(&self, state: &mut H) {
            0u8.hash(state);
        }
    }

    #[test]
    fn unique_roster_matches_push_unique_for_colliding_hashes() {
        let input = [4, 2, 4, 9, 2, 7, 9, 4];
        let mut roster = UniqueRoster::default();
        let mut expected = Vec::new();
        for value in input {
            roster.push(Colliding(value));
            push_unique(&mut expected, Colliding(value));
        }
        assert_eq!(roster.as_slice(), expected.as_slice());
        assert_eq!(roster.into_vec(), expected);
    }
}
