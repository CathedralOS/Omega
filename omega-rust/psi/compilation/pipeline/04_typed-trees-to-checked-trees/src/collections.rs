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

#[cfg(test)]
mod tests {
    use super::{push_unique, unique_in_order};

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
}
