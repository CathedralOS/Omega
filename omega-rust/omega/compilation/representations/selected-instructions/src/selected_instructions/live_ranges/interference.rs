//! Ordered raw interference edges with each consecutive lower ID stored once.
//!
//! This storage does not sort, deduplicate, validate or admit its rows. Iteration
//! and identity retain every input pair, including malformed or repeated pairs.

use std::cmp::Ordering;
use std::fmt;

use super::VirtualInterference;
use crate::VirtualRegisterId;

/// A raw pair sequence, compressed only by consecutive equal lower IDs.
/// Offsets use usize and every register ID retains its complete u32 value.
#[derive(Clone, Default)]
pub struct VirtualInterferences {
    lower_ids: Vec<VirtualRegisterId>,
    run_ends: Vec<usize>,
    higher_ids: Vec<VirtualRegisterId>,
}

impl VirtualInterferences {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.higher_ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.higher_ids.is_empty()
    }

    pub fn clear(&mut self) {
        self.lower_ids.clear();
        self.run_ends.clear();
        self.higher_ids.clear();
    }

    pub fn push(&mut self, pair: VirtualInterference) {
        self.higher_ids.push(pair.higher);
        let end = self.len();
        if self.lower_ids.last() == Some(&pair.lower) {
            *self.run_ends.last_mut().expect("lower run has an end") = end;
        } else {
            self.lower_ids.push(pair.lower);
            self.run_ends.push(end);
        }
    }

    pub fn get(&self, index: usize) -> Option<VirtualInterference> {
        let higher = *self.higher_ids.get(index)?;
        let run = self.run_ends.partition_point(|end| *end <= index);
        Some(VirtualInterference {
            lower: self.lower_ids[run],
            higher,
        })
    }

    pub fn iter(&self) -> Interferences<'_> {
        Interferences {
            rows: self,
            index: 0,
            run: 0,
        }
    }

    /// The same probe sequence as slice binary search, over reconstructed pairs.
    /// As with a slice, the caller must supply sorted rows for a meaningful
    /// membership result. Raw unsorted rows are retained without normalization.
    pub fn binary_search(&self, pair: &VirtualInterference) -> Result<usize, usize> {
        let mut size = self.len();
        if size == 0 {
            return Err(0);
        }
        let mut base = 0;
        while size > 1 {
            let half = size / 2;
            let mid = base + half;
            let cmp = self
                .get(mid)
                .expect("search position is in range")
                .cmp(pair);
            base = if cmp == Ordering::Greater { base } else { mid };
            size -= half;
        }
        let cmp = self
            .get(base)
            .expect("search position is in range")
            .cmp(pair);
        if cmp == Ordering::Equal {
            Ok(base)
        } else {
            Err(base + usize::from(cmp == Ordering::Less))
        }
    }

    /// Sort only newly appended pairs whose lower ID is identical. The overlap
    /// producer establishes that invariant for each source row, including when
    /// adjacent source rows happen to share their raw lower ID.
    pub fn sort_lower_tail(&mut self, start: usize) {
        assert!(start <= self.len());
        if start == self.len() {
            return;
        }
        let run = self.run_ends.partition_point(|end| *end <= start);
        assert_eq!(run + 1, self.lower_ids.len(), "tail must have one lower ID");
        self.higher_ids[start..].sort_unstable();
    }

    /// Preserve the producer's existing general path for noncanonical register
    /// order. Canonical source rows append directly without this temporary Vec.
    pub fn sort_unstable_and_dedup(&mut self) {
        let mut pairs: Vec<_> = self.iter().collect();
        pairs.sort_unstable();
        pairs.dedup();
        *self = pairs.into_iter().collect();
    }
}

impl FromIterator<VirtualInterference> for VirtualInterferences {
    fn from_iter<T: IntoIterator<Item = VirtualInterference>>(rows: T) -> Self {
        let mut result = Self::new();
        for row in rows {
            result.push(row);
        }
        result
    }
}

impl From<Vec<VirtualInterference>> for VirtualInterferences {
    fn from(rows: Vec<VirtualInterference>) -> Self {
        rows.into_iter().collect()
    }
}

impl PartialEq for VirtualInterferences {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}

impl Eq for VirtualInterferences {}

impl fmt::Debug for VirtualInterferences {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

pub struct Interferences<'a> {
    rows: &'a VirtualInterferences,
    index: usize,
    run: usize,
}

impl Iterator for Interferences<'_> {
    type Item = VirtualInterference;

    fn next(&mut self) -> Option<Self::Item> {
        let higher = *self.rows.higher_ids.get(self.index)?;
        while self.index >= self.rows.run_ends[self.run] {
            self.run += 1;
        }
        let lower = self.rows.lower_ids[self.run];
        self.index += 1;
        Some(VirtualInterference { lower, higher })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.rows.len() - self.index;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for Interferences<'_> {}
impl std::iter::FusedIterator for Interferences<'_> {}

impl<'a> IntoIterator for &'a VirtualInterferences {
    type Item = VirtualInterference;
    type IntoIter = Interferences<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests;
