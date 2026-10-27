//! Indexed storage for the difference graph; arithmetic stays in its owner.
//!
//! Stable row indices avoid repeated string-key tree walks during closure.
//! A separate lexical inventory retains the old graph's iteration order,
//! including inconsistent closure. Sparse rows allocate only recorded edges.
//! Names, ordering and rows clone/restore as one value for speculative updates.

use super::super::BigInt;

#[derive(Clone, Default)]
pub(in super::super) struct DifferenceBounds {
    names: Vec<String>,
    lexical_order: Vec<usize>,
    rows: Vec<Vec<(usize, BigInt)>>,
}

impl DifferenceBounds {
    pub(in super::super) fn atom_names(&self) -> impl Iterator<Item = &str> {
        self.lexical_order
            .iter()
            .map(|index| self.names[*index].as_str())
    }

    pub(super) fn lexical_indices(&self) -> &[usize] {
        &self.lexical_order
    }

    pub(super) fn position(&self, name: &str) -> Option<usize> {
        self.lexical_order
            .binary_search_by(|index| self.names[*index].as_str().cmp(name))
            .ok()
            .map(|position| self.lexical_order[position])
    }

    pub(super) fn intern(&mut self, name: &str) -> usize {
        match self
            .lexical_order
            .binary_search_by(|index| self.names[*index].as_str().cmp(name))
        {
            Ok(position) => self.lexical_order[position],
            Err(position) => {
                let index = self.names.len();
                self.names.push(name.to_owned());
                self.rows.push(Vec::new());
                self.lexical_order.insert(position, index);
                index
            }
        }
    }

    /// Stored cells deliberately do not supply a virtual zero diagonal.
    pub(super) fn stored_bound(&self, from: &str, to: &str) -> Option<&BigInt> {
        self.stored_bound_indices(self.position(from)?, self.position(to)?)
    }

    pub(super) fn stored_bound_indices(&self, from: usize, to: usize) -> Option<&BigInt> {
        let row = &self.rows[from];
        row.binary_search_by_key(&to, |(destination, _)| *destination)
            .ok()
            .map(|position| &row[position].1)
    }

    pub(super) fn stored_targets(&self, from: usize) -> impl Iterator<Item = usize> + '_ {
        self.rows[from].iter().map(|(destination, _)| *destination)
    }

    pub(super) fn record(&mut self, from: &str, to: &str, bound: BigInt) -> bool {
        // Both vertices existed after every old record attempt, even when a
        // stored bound did not improve or the seed was a negative diagonal.
        let from = self.intern(from);
        let to = self.intern(to);
        self.record_indices(from, to, bound)
    }

    pub(super) fn record_indices(&mut self, from: usize, to: usize, bound: BigInt) -> bool {
        let row = &mut self.rows[from];
        match row.binary_search_by_key(&to, |(destination, _)| *destination) {
            Ok(position) => {
                if bound > row[position].1 {
                    row[position].1 = bound;
                    true
                } else {
                    false
                }
            }
            Err(position) => {
                row.insert(position, (to, bound));
                true
            }
        }
    }
}
