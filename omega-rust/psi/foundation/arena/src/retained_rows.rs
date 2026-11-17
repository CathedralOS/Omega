//! Ordered side records retain immutable batches through a continuation.
//! The logical roster has Vec order; RootedArena owns record identity and
//! copies only a record that is actually rewritten. Forking or appending may
//! copy roster/association metadata, but never the retained record payload.

use std::fmt;
use std::ops::{Index, IndexMut};
use std::sync::Arc;

use crate::{Arena, Handle, RootedArena};

type Row<T> = Handle<Option<T>>;

pub struct RetainedRows<T> {
    records: RootedArena<Option<T>>,
    order: Arc<Vec<Row<T>>>,
}

impl<T> Default for RetainedRows<T> {
    fn default() -> Self {
        Self {
            records: RootedArena::default(),
            order: Arc::new(Vec::new()),
        }
    }
}

impl<T> Clone for RetainedRows<T> {
    fn clone(&self) -> Self {
        Self {
            records: self.records.clone(),
            order: Arc::clone(&self.order),
        }
    }
}

impl<T> RetainedRows<T> {
    #[inline]
    pub fn len(&self) -> usize {
        self.order.len()
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }
    #[inline]
    fn row(&self, handle: Row<T>) -> &T {
        self.records
            .get(handle)
            .as_ref()
            .expect("retained row roster names an empty record")
    }
    #[inline]
    pub fn get(&self, position: usize) -> Option<&T> {
        self.order.get(position).map(|handle| self.row(*handle))
    }
    #[inline]
    pub fn first(&self) -> Option<&T> {
        self.get(0)
    }
    #[inline]
    pub fn last(&self) -> Option<&T> {
        self.order.last().map(|handle| self.row(*handle))
    }
    #[inline]
    pub fn iter(&self) -> RetainedRowsIter<'_, T> {
        RetainedRowsIter {
            owner: self,
            positions: self.order.iter(),
        }
    }
    pub fn push(&mut self, value: T) {
        let row = self.records.insert(Some(value));
        Arc::make_mut(&mut self.order).push(row);
    }
    pub fn insert(&mut self, position: usize, value: T) {
        assert!(
            position <= self.len(),
            "retained row insertion is out of range"
        );
        let row = self.records.insert(Some(value));
        Arc::make_mut(&mut self.order).insert(position, row);
    }
    #[inline]
    pub fn partition_point(&self, mut predicate: impl FnMut(&T) -> bool) -> usize {
        self.order
            .partition_point(|handle| predicate(self.row(*handle)))
    }
    #[inline]
    pub fn binary_search_by_key<K: Ord>(
        &self,
        key: &K,
        mut read: impl FnMut(&T) -> K,
    ) -> Result<usize, usize> {
        self.order
            .binary_search_by_key(key, |handle| read(self.row(*handle)))
    }
    #[inline]
    pub fn contains(&self, value: &T) -> bool
    where
        T: PartialEq,
    {
        self.iter().any(|row| row == value)
    }
    #[inline]
    pub fn starts_with(&self, prefix: &Self) -> bool
    where
        T: PartialEq,
    {
        self.len() >= prefix.len()
            && self
                .iter()
                .zip(prefix.iter())
                .all(|(left, right)| left == right)
    }
    pub fn retain(&mut self, mut keep: impl FnMut(&T) -> bool) {
        let Some(first_removed) = self
            .order
            .iter()
            .position(|handle| !keep(self.row(*handle)))
        else {
            return;
        };
        let mut order = Vec::with_capacity(self.len() - 1);
        let mut removed = vec![self.order[first_removed]];
        order.extend_from_slice(&self.order[..first_removed]);
        // Keep the owner unchanged if a predicate unwinds. Only after every
        // decision do we publish the roster and retire its removed records.
        for handle in &self.order[first_removed + 1..] {
            if keep(self.row(*handle)) {
                order.push(*handle);
            } else {
                removed.push(*handle);
            }
        }
        self.order = Arc::new(order);
        for handle in removed {
            assert!(self.records.free(handle));
        }
    }

    pub fn truncate(&mut self, length: usize) {
        while self.len() > length {
            let handle = Arc::make_mut(&mut self.order)
                .pop()
                .expect("nonempty retained roster");
            assert!(self.records.free(handle));
        }
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn resize_with(&mut self, length: usize, mut create: impl FnMut() -> T) {
        self.truncate(length);
        while self.len() < length {
            self.push(create());
        }
    }
}

impl<T: Clone> RetainedRows<T> {
    pub fn get_mut(&mut self, position: usize) -> Option<&mut T> {
        let handle = *self.order.get(position)?;
        Some(
            self.records
                .get_mut(handle)
                .as_mut()
                .expect("retained row roster names an empty record"),
        )
    }
    pub fn resize(&mut self, length: usize, value: T) {
        self.resize_with(length, || value.clone());
    }
}

impl<T> Index<usize> for RetainedRows<T> {
    type Output = T;
    #[inline]
    fn index(&self, position: usize) -> &T {
        self.get(position)
            .expect("retained row index is out of range")
    }
}
impl<T: Clone> IndexMut<usize> for RetainedRows<T> {
    fn index_mut(&mut self, position: usize) -> &mut T {
        self.get_mut(position)
            .expect("retained row index is out of range")
    }
}
impl<T> Extend<T> for RetainedRows<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, values: I) {
        for value in values {
            self.push(value);
        }
    }
}
impl<T> From<Vec<T>> for RetainedRows<T> {
    fn from(values: Vec<T>) -> Self {
        values.into_iter().collect()
    }
}
impl<T> FromIterator<T> for RetainedRows<T> {
    fn from_iter<I: IntoIterator<Item = T>>(values: I) -> Self {
        let values = values.into_iter().map(Some).collect::<Vec<_>>();
        let length = values.len();
        let records = Arena::from_storage(None, values, None, 1, true)
            .expect("retained row count exceeds slot identity")
            .into();
        let order = (0..length)
            .map(|position| {
                Row::from_parts(
                    u32::try_from(position + 1).expect("retained row count exceeds slot identity"),
                    1,
                )
            })
            .collect();
        Self {
            records,
            order: Arc::new(order),
        }
    }
}
impl<T: fmt::Debug> fmt::Debug for RetainedRows<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_list().entries(self.iter()).finish()
    }
}
impl<T: PartialEq> PartialEq for RetainedRows<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}
impl<T: Eq> Eq for RetainedRows<T> {}

pub struct RetainedRowsIter<'a, T> {
    owner: &'a RetainedRows<T>,
    positions: std::slice::Iter<'a, Row<T>>,
}
impl<'a, T> Iterator for RetainedRowsIter<'a, T> {
    type Item = &'a T;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.positions.next().map(|handle| self.owner.row(*handle))
    }
    #[inline]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.positions.nth(n).map(|handle| self.owner.row(*handle))
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.positions.size_hint()
    }
}
impl<T> DoubleEndedIterator for RetainedRowsIter<'_, T> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.positions
            .next_back()
            .map(|handle| self.owner.row(*handle))
    }
}
impl<T> ExactSizeIterator for RetainedRowsIter<'_, T> {}
impl<T> std::iter::FusedIterator for RetainedRowsIter<'_, T> {}
impl<'a, T> IntoIterator for &'a RetainedRows<T> {
    type Item = &'a T;
    type IntoIter = RetainedRowsIter<'a, T>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Tracked {
        value: String,
        copies: Arc<AtomicUsize>,
    }
    impl Clone for Tracked {
        fn clone(&self) -> Self {
            self.copies.fetch_add(1, Ordering::Relaxed);
            Self {
                value: self.value.clone(),
                copies: Arc::clone(&self.copies),
            }
        }
    }

    #[test]
    fn fork_append_and_sorted_insertion_keep_retained_records() {
        let copies = Arc::new(AtomicUsize::new(0));
        let make = |value: String| Tracked {
            value,
            copies: Arc::clone(&copies),
        };
        let mut rows: RetainedRows<_> = (0..260).map(|value| make(value.to_string())).collect();
        let predecessor = rows.clone();
        rows.push(make("tail".into()));
        rows.insert(1, make("inserted".into()));
        assert_eq!(copies.load(Ordering::Relaxed), 0);
        assert!(std::ptr::eq(&predecessor[0], &rows[0]));
        assert!(std::ptr::eq(&predecessor[1], &rows[2]));

        rows[0].value.push_str(" changed");
        assert_eq!(copies.load(Ordering::Relaxed), 1);
        assert_eq!(predecessor[0].value, "0");
        assert_eq!(rows[0].value, "0 changed");
        assert!(std::ptr::eq(&predecessor[1], &rows[2]));

        let mut visits = 0;
        rows.retain(|row| {
            visits += 1;
            row.value != "inserted"
        });
        assert_eq!(visits, 262);
        assert_eq!(copies.load(Ordering::Relaxed), 1);
        assert!(std::ptr::eq(&predecessor[1], &rows[1]));
        assert_eq!(rows.last().unwrap().value, "tail");
    }

    #[test]
    fn a_panicking_retention_predicate_keeps_the_original_roster() {
        let mut rows = RetainedRows::from(vec![
            "first".to_owned(),
            "second".to_owned(),
            "third".to_owned(),
        ]);
        let retained = rows.clone();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            rows.retain(|row| {
                assert_ne!(row, "third", "predicate stopped");
                row != "first"
            });
        }));
        assert!(result.is_err());
        assert_eq!(rows, retained);
        assert!(std::ptr::eq(&rows[0], &retained[0]));
    }

    #[test]
    fn retaining_every_row_leaves_the_roster_shared() {
        let mut rows = RetainedRows::from(vec!["first".to_owned(), "second".to_owned()]);
        let roster = Arc::clone(&rows.order);
        let mut visits = 0;
        rows.retain(|_| {
            visits += 1;
            true
        });
        assert_eq!(visits, 2);
        assert!(Arc::ptr_eq(&rows.order, &roster));
    }

    #[test]
    fn legacy_vector_bytes_and_decoded_allocation_limits_are_preserved() {
        let values = vec!["first".to_owned(), "second".to_owned()];
        let rows = RetainedRows::from(values.clone());
        let limits = compiler_storage::RecordLimits::default();
        let bytes = compiler_storage::encode(&values, limits).unwrap();
        assert_eq!(compiler_storage::encode(&rows, limits).unwrap(), bytes);
        let decoded: RetainedRows<String> = compiler_storage::decode(&bytes, limits).unwrap();
        assert_eq!(decoded, rows);

        let tight = compiler_storage::RecordLimits {
            allocation: 128,
            ..limits
        };
        assert!(compiler_storage::decode::<Vec<String>>(&bytes, tight).is_ok());
        let error = compiler_storage::decode::<RetainedRows<String>>(&bytes, tight).unwrap_err();
        assert!(error.to_string().contains("allocation limit"), "{error}");
    }

    #[test]
    #[should_panic(expected = "unresolved handle")]
    fn a_stale_logical_roster_cannot_read_a_reused_row() {
        let mut rows = RetainedRows::from(vec!["old".to_owned()]);
        let predecessor = rows.clone();
        let stale = rows.order[0];
        rows.clear();
        rows.push("new".into());
        assert_eq!(predecessor[0], "old");
        // Give this owner an explicitly retired/reused slot, rather than
        // relying on a fresh owner's coincidentally equal slot number.
        let retired = rows.order[0];
        rows.retain(|_| false);
        rows.push("replacement".into());
        assert_eq!(rows.order[0].arena_index(), retired.arena_index());
        assert_ne!(rows.order[0].generation(), retired.generation());
        assert!(predecessor.records.is_valid(stale));
        Arc::make_mut(&mut rows.order)[0] = retired;
        let _ = &rows[0];
    }
}

impl<T: compiler_storage::Record> compiler_storage::Record for RetainedRows<T> {
    fn write(
        &self,
        output: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        // This is the previous Vec<T> encoding, not the internal optional rows
        // or logical handles. Neither is another semantic authority on ingress.
        output.record(|output| {
            output.sequence(self.len())?;
            for value in self {
                value.write(output)?;
            }
            Ok(())
        })
    }
    fn read<Context>(
        input: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        input.record(|input| {
            // Decode straight into the adopted batch. Charging Option<T> here
            // covers its real allocation without a transient old Vec<T>.
            let length = input.sequence::<Option<T>>()?;
            if length > u32::MAX as usize {
                return Err(input.error("retained row count exceeds slot identity"));
            }
            std::alloc::Layout::array::<Option<T>>(length)
                .map_err(|_| input.error("retained row payload layout overflow"))?;
            std::alloc::Layout::array::<Row<T>>(length)
                .map_err(|_| input.error("retained row roster layout overflow"))?;
            let metadata = RootedArena::<Option<T>>::decoded_record_metadata_bytes(length)
                .and_then(|bytes| {
                    length
                        .checked_mul(std::mem::size_of::<Row<T>>())
                        .and_then(|roster| bytes.checked_add(roster))
                })
                .and_then(|bytes| {
                    compiler_storage::shared_allocation_bytes(
                        std::alloc::Layout::new::<Vec<Row<T>>>(),
                    )
                    .and_then(|roster| bytes.checked_add(roster))
                })
                .ok_or_else(|| input.error("retained row metadata allocation overflow"))?;
            input.allocate(metadata)?;
            let mut values = Vec::with_capacity(length);
            for _ in 0..length {
                values.push(Some(T::read(input)?));
            }
            let records = Arena::from_storage(None, values, None, 1, true)
                .map_err(|reason| input.error(reason))?
                .into();
            let order = (0..length)
                .map(|position| Row::from_parts((position + 1) as u32, 1))
                .collect();
            Ok(Self {
                records,
                order: Arc::new(order),
            })
        })
    }
}
