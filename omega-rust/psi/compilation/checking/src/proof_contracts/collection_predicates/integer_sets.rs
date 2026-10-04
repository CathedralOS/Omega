//! Exact finite unions of integer ranges.
//!
//! Predicate unfolding refines one value at a time by comparisons against
//! other known values. A plain interval cannot represent the false side of
//! `value == 0 || value == 1`, so each value keeps a sorted union of
//! disjoint, non-adjacent inclusive ranges instead. Every operation is exact;
//! only the explorer's work budget bounds how many values it refines.

use typed_trees::expression::BinaryOperator;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IntegerSet {
    /// Sorted, disjoint, non-adjacent inclusive ranges.
    ranges: Vec<(i128, i128)>,
}

impl IntegerSet {
    pub(crate) fn empty() -> Self {
        Self { ranges: Vec::new() }
    }

    pub(crate) fn range(low: i128, high: i128) -> Self {
        if low > high {
            return Self::empty();
        }
        Self {
            ranges: vec![(low, high)],
        }
    }

    pub(crate) fn singleton(value: i128) -> Self {
        Self::range(value, value)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    pub(crate) fn single_value(&self) -> Option<i128> {
        match self.ranges.as_slice() {
            [(low, high)] if low == high => Some(*low),
            _ => None,
        }
    }

    /// The smallest and largest member.
    pub(crate) fn bounds(&self) -> Option<(i128, i128)> {
        Some((self.ranges.first()?.0, self.ranges.last()?.1))
    }

    pub(crate) fn union(&self, other: &Self) -> Self {
        let mut ranges = self.ranges.clone();
        ranges.extend_from_slice(&other.ranges);
        Self::normalized(ranges)
    }

    pub(crate) fn intersect(&self, other: &Self) -> Self {
        let mut ranges = Vec::new();
        for (left_low, left_high) in &self.ranges {
            for (right_low, right_high) in &other.ranges {
                let low = (*left_low).max(*right_low);
                let high = (*left_high).min(*right_high);
                if low <= high {
                    ranges.push((low, high));
                }
            }
        }
        Self::normalized(ranges)
    }

    /// Members of `self` that are not members of `other`.
    pub(crate) fn difference(&self, other: &Self) -> Self {
        let mut remaining = self.ranges.clone();
        for (cut_low, cut_high) in &other.ranges {
            let mut next = Vec::with_capacity(remaining.len() + 1);
            for (low, high) in remaining {
                if high < *cut_low || *cut_high < low {
                    next.push((low, high));
                    continue;
                }
                if low < *cut_low {
                    next.push((low, cut_low - 1));
                }
                if *cut_high < high {
                    next.push((cut_high + 1, high));
                }
            }
            remaining = next;
        }
        Self::normalized(remaining)
    }

    pub(crate) fn is_subset_of(&self, other: &Self) -> bool {
        self.difference(other).is_empty()
    }

    /// Every integer standing in `operator` to `bound`, for the builtin
    /// integer comparisons. The universe is the full `i128` range; callers
    /// intersect with the carrier-bounded set they refine.
    pub(crate) fn satisfying(operator: BinaryOperator, bound: i128) -> Option<Self> {
        Some(match operator {
            BinaryOperator::Equal => Self::singleton(bound),
            BinaryOperator::NotEqual => {
                Self::range(i128::MIN, i128::MAX).difference(&Self::singleton(bound))
            }
            BinaryOperator::Less => match bound.checked_sub(1) {
                Some(high) => Self::range(i128::MIN, high),
                None => Self::empty(),
            },
            BinaryOperator::LessOrEqual => Self::range(i128::MIN, bound),
            BinaryOperator::Greater => match bound.checked_add(1) {
                Some(low) => Self::range(low, i128::MAX),
                None => Self::empty(),
            },
            BinaryOperator::GreaterOrEqual => Self::range(bound, i128::MAX),
            _ => return None,
        })
    }

    /// The number of ranges, which bounds the representation size.
    pub(crate) fn range_count(&self) -> usize {
        self.ranges.len()
    }

    fn normalized(mut ranges: Vec<(i128, i128)>) -> Self {
        ranges.retain(|(low, high)| low <= high);
        ranges.sort_unstable();
        let mut merged: Vec<(i128, i128)> = Vec::with_capacity(ranges.len());
        for (low, high) in ranges {
            if let Some(last) = merged.last_mut()
                && low <= last.1.saturating_add(1)
            {
                last.1 = last.1.max(high);
                continue;
            }
            merged.push((low, high));
        }
        Self { ranges: merged }
    }
}
