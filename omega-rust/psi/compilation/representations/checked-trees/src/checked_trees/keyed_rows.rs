//! Rows of one durable arena, grouped by a key their producer records.
//!
//! Several checked tables answer the same question in different vocabularies:
//! which rows belong to this state, machine, expression or caller coordinate.
//! Value plans bucket their roots by owning state, ownership facts bucket
//! permission events by state, and proof facts bucket calls and exits by
//! caller coordinate. Every answer must stay exactly as fresh as the arena it
//! derives from, and every table used to re-implement that relationship — its
//! own map, its own build loop, its own revision key.
//!
//! [`KeyedRows`] owns the relationship once. It builds the grouping from a
//! durable arena's exact revision and rebuilds when that revision moves, so a
//! table declares one field and one key projection instead of another index.
//! [`DurableRows`] names the two arena reads the grouping needs, so the same
//! mechanism serves the contiguous and paged row storage alike.

use arena::{Arena, Handle, OwnedIndex, PagedArenaSet};
use std::hash::Hash;
use symbols::SymbolKeyMap;

/// The durable arena reads a row grouping needs: the exact revision that
/// moves on every mutation-capable call, and every row in arena order.
pub trait DurableRows {
    type Row;

    fn revision(&self) -> u64;

    fn rows(&self) -> impl Iterator<Item = (Handle<Self::Row>, &Self::Row)>;
}

impl<T: Default> DurableRows for Arena<T> {
    type Row = T;

    fn revision(&self) -> u64 {
        Arena::revision(self)
    }

    fn rows(&self) -> impl Iterator<Item = (Handle<T>, &T)> {
        Arena::iter(self)
    }
}

impl<T: Default> DurableRows for PagedArenaSet<T> {
    type Row = T;

    fn revision(&self) -> u64 {
        PagedArenaSet::revision(self)
    }

    fn rows(&self) -> impl Iterator<Item = (Handle<T>, &T)> {
        PagedArenaSet::iter(self)
    }
}

/// One table's rows grouped by a key derived from each row, rebuilt whenever
/// the producing arena's revision moves. The query borrows the arena that
/// produced the grouping, so the index cannot be keyed against one arena's
/// revision and read as another's.
pub struct KeyedRows<Key, Row> {
    groups: OwnedIndex<SymbolKeyMap<Key, Vec<Handle<Row>>>>,
}

impl<Key: Eq + Hash, Row> KeyedRows<Key, Row> {
    /// Run `run` against the whole grouping: every row's handle under the key
    /// `key_of` derives from it, in arena order. Batch insertion and mutation
    /// have independent revision/length effects; only the arena's exact
    /// revision establishes freshness of every bucket.
    pub fn with_groups<A, R>(
        &self,
        arena: &A,
        key_of: impl FnMut(&Row) -> Key,
        run: impl FnOnce(&SymbolKeyMap<Key, Vec<Handle<Row>>>) -> R,
    ) -> R
    where
        A: DurableRows<Row = Row>,
    {
        let mut key_of = key_of;
        self.groups.with(
            arena.revision(),
            || {
                let mut groups: SymbolKeyMap<Key, Vec<Handle<Row>>> = SymbolKeyMap::default();
                for (handle, row) in arena.rows() {
                    groups.entry(key_of(row)).or_default().push(handle);
                }
                groups
            },
            run,
        )
    }

    /// Run `run` against the rows recorded under `key`, in arena order.
    pub fn with_rows<A, R>(
        &self,
        arena: &A,
        key_of: impl FnMut(&Row) -> Key,
        key: &Key,
        run: impl FnOnce(&[Handle<Row>]) -> R,
    ) -> R
    where
        A: DurableRows<Row = Row>,
    {
        self.with_groups(arena, key_of, |groups| {
            run(groups.get(key).map_or(&[], Vec::as_slice))
        })
    }
}

impl<Key, Row> KeyedRows<Key, Row> {
    /// Drop the built grouping; the next query rebuilds it. A table whose
    /// arena can be replaced through mutable access clears the grouping so
    /// the replacement cannot inherit its predecessor's buckets.
    pub fn clear(&mut self) {
        self.groups.clear();
    }
}

impl<Key, Row> Default for KeyedRows<Key, Row> {
    fn default() -> Self {
        Self {
            groups: OwnedIndex::default(),
        }
    }
}

impl<Key: Clone, Row> Clone for KeyedRows<Key, Row> {
    fn clone(&self) -> Self {
        Self {
            groups: self.groups.clone(),
        }
    }
}

// Written out so the derived grouping stays out of the facts' rendering.
impl<Key, Row> std::fmt::Debug for KeyedRows<Key, Row> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("KeyedRows")
    }
}

// The grouping is derived content: two tables with equal rows are equal
// whether or not their group maps have been built, exactly as `OwnedIndex`
// already holds for the index it owns.
impl<Key, Row> PartialEq for KeyedRows<Key, Row> {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl<Key, Row> Eq for KeyedRows<Key, Row> {}

#[cfg(test)]
mod tests {
    use super::{Arena, Handle, KeyedRows};
    use symbols::SymbolHandle;

    #[derive(Clone, Default, PartialEq, Eq)]
    struct Row {
        owner: SymbolHandle,
        marker: u32,
    }

    fn rows(
        keyed: &KeyedRows<SymbolHandle, Row>,
        arena: &Arena<Row>,
        owner: SymbolHandle,
    ) -> Vec<Handle<Row>> {
        keyed.with_rows(arena, |row| row.owner, &owner, |rows| rows.to_vec())
    }

    #[test]
    fn groups_stay_fresh_across_mutation_and_batch_append() {
        let keyed = KeyedRows::<SymbolHandle, Row>::default();
        let original = SymbolHandle::from_parts(1, 1);
        let moved = SymbolHandle::from_parts(2, 1);
        let mut arena = Arena::<Row>::default();
        assert!(rows(&keyed, &arena, original).is_empty());
        let first = arena.insert(Row {
            owner: original,
            marker: 1,
        });
        assert_eq!(rows(&keyed, &arena, original), [first]);
        arena.get_mut(first).owner = moved;
        assert_eq!(rows(&keyed, &arena, moved), [first]);
        assert!(rows(&keyed, &arena, original).is_empty());
        let batch = arena.insert_many([
            Row {
                owner: original,
                marker: 2,
            },
            Row {
                owner: original,
                marker: 3,
            },
        ]);
        assert_eq!(rows(&keyed, &arena, moved), [first]);
        assert_eq!(
            rows(&keyed, &arena, original),
            batch.handles().collect::<Vec<_>>()
        );
        assert!(arena.free(first));
        assert!(rows(&keyed, &arena, moved).is_empty());
        assert_eq!(rows(&keyed, &arena, original).len(), 2);
    }
}
