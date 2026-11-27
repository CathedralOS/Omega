//! Side rows owned by one declaration each, sorted by owner.
//!
//! Passes ask for one machine's rows many times per compilation, so the rows
//! stay sorted by their owner's symbol and a query is a binary search, not a
//! scan of every row. Rows are only ever added through `push`, which inserts
//! after the rows of equal owner, so one owner keeps its rows in insertion
//! order. A decoded roster that is out of order is refused rather than trusted,
//! because a search over it would silently miss rows.

use std::ops::{Index, IndexMut};
use symbols::SymbolHandle;

/// A row that belongs to exactly one declaration.
pub trait OwnedRow {
    fn row_owner(&self) -> SymbolHandle;
}

fn owner_key(owner: SymbolHandle) -> (u32, u32) {
    (owner.arena_index(), owner.generation())
}

#[derive(Debug, PartialEq, Eq)]
pub struct OwnerSortedRows<T> {
    rows: arena::RetainedRows<T>,
}

impl<T> Default for OwnerSortedRows<T> {
    fn default() -> Self {
        Self {
            rows: arena::RetainedRows::default(),
        }
    }
}

impl<T> Clone for OwnerSortedRows<T> {
    fn clone(&self) -> Self {
        Self {
            rows: self.rows.clone(),
        }
    }
}

impl<T: OwnedRow> OwnerSortedRows<T> {
    #[inline]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    #[inline]
    pub fn iter(&self) -> arena::RetainedRowsIter<'_, T> {
        self.rows.iter()
    }

    #[inline]
    pub fn get(&self, position: usize) -> Option<&T> {
        self.rows.get(position)
    }

    pub fn push(&mut self, row: T) {
        let key = owner_key(row.row_owner());
        let position = self
            .rows
            .partition_point(|existing| owner_key(existing.row_owner()) <= key);
        self.rows.insert(position, row);
    }

    /// The positions of `owner`'s rows, in insertion order.
    pub fn positions_owned_by(&self, owner: SymbolHandle) -> std::ops::Range<usize> {
        let key = owner_key(owner);
        let start = self
            .rows
            .partition_point(|row| owner_key(row.row_owner()) < key);
        let end = self
            .rows
            .partition_point(|row| owner_key(row.row_owner()) <= key);
        start..end
    }

    pub fn owned_by(&self, owner: SymbolHandle) -> impl Iterator<Item = &T> {
        self.positions_owned_by(owner)
            .map(move |position| &self.rows[position])
    }

    pub fn retain(&mut self, keep: impl FnMut(&T) -> bool) {
        self.rows.retain(keep);
    }
}

impl<T: OwnedRow + Clone> OwnerSortedRows<T> {
    /// A row's owner decides its position, so a caller may rewrite any field
    /// but the owner.
    pub fn get_mut(&mut self, position: usize) -> Option<&mut T> {
        self.rows.get_mut(position)
    }
}

impl<T> Index<usize> for OwnerSortedRows<T> {
    type Output = T;
    #[inline]
    fn index(&self, position: usize) -> &T {
        &self.rows[position]
    }
}

impl<T: Clone> IndexMut<usize> for OwnerSortedRows<T> {
    fn index_mut(&mut self, position: usize) -> &mut T {
        &mut self.rows[position]
    }
}

impl<T: OwnedRow> Extend<T> for OwnerSortedRows<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, rows: I) {
        for row in rows {
            self.push(row);
        }
    }
}

impl<T: OwnedRow> FromIterator<T> for OwnerSortedRows<T> {
    fn from_iter<I: IntoIterator<Item = T>>(rows: I) -> Self {
        let mut rows: Vec<T> = rows.into_iter().collect();
        // Stable, so rows of one owner keep the order they arrived in.
        rows.sort_by_key(|row| owner_key(row.row_owner()));
        Self { rows: rows.into() }
    }
}

impl<'a, T: OwnedRow> IntoIterator for &'a OwnerSortedRows<T> {
    type Item = &'a T;
    type IntoIter = arena::RetainedRowsIter<'a, T>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T: OwnedRow + record_storage::Record> record_storage::Record for OwnerSortedRows<T> {
    fn write(
        &self,
        output: &mut record_storage::RecordWriter,
    ) -> Result<(), record_storage::RecordError> {
        self.rows.write(output)
    }

    fn read<Context>(
        input: &mut record_storage::RecordReader<'_, Context>,
    ) -> Result<Self, record_storage::RecordError> {
        let rows = arena::RetainedRows::<T>::read(input)?;
        let sorted = rows
            .iter()
            .zip(rows.iter().skip(1))
            .all(|(left, right)| owner_key(left.row_owner()) <= owner_key(right.row_owner()));
        if !sorted {
            return Err(input.error("owner rows are not sorted by owner"));
        }
        Ok(Self { rows })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Row {
        owner: SymbolHandle,
        label: u32,
    }

    impl OwnedRow for Row {
        fn row_owner(&self) -> SymbolHandle {
            self.owner
        }
    }

    fn symbol(index: u32) -> SymbolHandle {
        SymbolHandle::from_parts(index, 1)
    }

    #[test]
    fn each_owner_finds_exactly_its_rows_in_insertion_order() {
        let mut rows = OwnerSortedRows::default();
        for (owner, label) in [(3, 0), (1, 1), (3, 2), (2, 3), (1, 4)] {
            rows.push(Row {
                owner: symbol(owner),
                label,
            });
        }
        let labels = |owner| {
            rows.owned_by(symbol(owner))
                .map(|row: &Row| row.label)
                .collect::<Vec<_>>()
        };
        assert_eq!(labels(1), [1, 4]);
        assert_eq!(labels(2), [3]);
        assert_eq!(labels(3), [0, 2]);
        assert!(labels(4).is_empty());

        let collected: OwnerSortedRows<Row> = rows.iter().rev().cloned().collect();
        assert_eq!(
            collected
                .owned_by(symbol(1))
                .map(|row| row.label)
                .collect::<Vec<_>>(),
            [4, 1]
        );
    }
}
