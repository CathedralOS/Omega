//! Dense numeric-keyed tables and bitsets, independent of any language IDs.

use std::marker::PhantomData;

pub trait NumericId: Copy + Eq + Ord + std::hash::Hash {
    fn new(raw: u64) -> Option<Self>;
    fn get(self) -> u64;
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct IdTable<Id, T> {
    first: u64,
    values: Vec<T>,
    identity: PhantomData<Id>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NonDenseIdentities {
    pub expected: u64,
    pub found: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdTableBuildError {
    RepeatedOrUnordered { previous: u64, found: u64 },
    SpanOverflow,
    SpanLimit { span: u64, maximum: usize },
    AllocationFailed,
}

impl<Id: NumericId, T> IdTable<Id, T> {
    pub fn from_rows(rows: impl IntoIterator<Item = (Id, T)>) -> Result<Self, NonDenseIdentities> {
        let mut rows = rows.into_iter();
        let Some((first, value)) = rows.next() else {
            return Ok(Self::default());
        };
        let first = first.get();
        let mut values = Vec::with_capacity(rows.size_hint().0 + 1);
        values.push(value);
        for (identity, value) in rows {
            let expected = first.checked_add(values.len() as u64);
            if expected != Some(identity.get()) {
                return Err(NonDenseIdentities {
                    expected: expected.unwrap_or(u64::MAX),
                    found: identity.get(),
                });
            }
            values.push(value);
        }
        Ok(Self {
            first,
            values,
            identity: PhantomData,
        })
    }

    pub fn from_rows_with_gaps(
        rows: impl IntoIterator<Item = (Id, T)>,
        maximum_span: usize,
    ) -> Result<Self, IdTableBuildError>
    where
        T: Default,
    {
        let mut rows = rows.into_iter();
        let Some((first, value)) = rows.next() else {
            return Ok(Self::default());
        };
        let first = first.get();
        if maximum_span == 0 {
            return Err(IdTableBuildError::SpanLimit {
                span: 1,
                maximum: maximum_span,
            });
        }
        let mut values = Vec::new();
        values
            .try_reserve(1)
            .map_err(|_| IdTableBuildError::AllocationFailed)?;
        values.push(value);
        let mut previous = first;
        for (identity, value) in rows {
            let raw = identity.get();
            if raw <= previous {
                return Err(IdTableBuildError::RepeatedOrUnordered {
                    previous,
                    found: raw,
                });
            }
            let span = raw
                .checked_sub(first)
                .and_then(|offset| offset.checked_add(1))
                .ok_or(IdTableBuildError::SpanOverflow)?;
            if span > maximum_span as u64 {
                return Err(IdTableBuildError::SpanLimit {
                    span,
                    maximum: maximum_span,
                });
            }
            let offset = raw
                .checked_sub(first)
                .and_then(|offset| usize::try_from(offset).ok())
                .ok_or(IdTableBuildError::SpanOverflow)?;
            let target_len = offset
                .checked_add(1)
                .ok_or(IdTableBuildError::SpanOverflow)?;
            values
                .try_reserve(target_len - values.len())
                .map_err(|_| IdTableBuildError::AllocationFailed)?;
            values.resize_with(offset, T::default);
            values.push(value);
            previous = raw;
        }
        Ok(Self {
            first,
            values,
            identity: PhantomData,
        })
    }

    /// Appends one row and mints its identity: the next in this table's run,
    /// starting at 1 when empty, so a table built by pushing is dense by
    /// construction.
    pub fn push(&mut self, value: T) -> Id {
        if self.values.is_empty() {
            self.first = 1;
        }
        let identity =
            Id::new(self.first + self.values.len() as u64).expect("a pushed identity is nonzero");
        self.values.push(value);
        identity
    }

    fn position(&self, identity: Id) -> Option<usize> {
        let offset = identity.get().checked_sub(self.first)?;
        usize::try_from(offset)
            .ok()
            .filter(|position| *position < self.values.len())
    }

    pub fn get(&self, identity: Id) -> Option<&T> {
        self.position(identity)
            .map(|position| &self.values[position])
    }

    pub fn get_mut(&mut self, identity: Id) -> Option<&mut T> {
        self.position(identity)
            .map(|position| &mut self.values[position])
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn values(&self) -> std::slice::Iter<'_, T> {
        self.values.iter()
    }

    /// Return the identity at one row ordinal without traversing earlier rows.
    pub fn identity_at(&self, position: usize) -> Option<Id> {
        self.values.get(position)?;
        Id::new(self.first.checked_add(u64::try_from(position).ok()?)?)
    }

    pub fn keys(&self) -> impl DoubleEndedIterator<Item = Id> + ExactSizeIterator + '_ {
        (0..self.values.len()).map(|offset| {
            Id::new(self.first + offset as u64).expect("table identities are nonzero")
        })
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (Id, &T)> + ExactSizeIterator + '_ {
        self.keys().zip(self.values.iter())
    }

    pub fn iter_mut(
        &mut self,
    ) -> impl DoubleEndedIterator<Item = (Id, &mut T)> + ExactSizeIterator + '_ {
        let first = self.first;
        self.values
            .iter_mut()
            .enumerate()
            .map(move |(offset, value)| {
                let identity =
                    Id::new(first + offset as u64).expect("table identities are nonzero");
                (identity, value)
            })
    }

    pub fn map<U>(&self, mut derive: impl FnMut(Id, &T) -> U) -> IdTable<Id, U> {
        IdTable {
            first: self.first,
            values: self
                .iter()
                .map(|(identity, value)| derive(identity, value))
                .collect(),
            identity: PhantomData,
        }
    }
}

impl<Id, T> Default for IdTable<Id, T> {
    fn default() -> Self {
        Self {
            first: 1,
            values: Vec::new(),
            identity: PhantomData,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdSet<Id> {
    words: Vec<u64>,
    identity: PhantomData<Id>,
}

impl<Id: NumericId> IdSet<Id> {
    pub fn new() -> Self {
        Self::default()
    }

    fn coordinate(identity: Id) -> (usize, u64) {
        let bit = identity.get() - 1;
        (
            usize::try_from(bit / 64).expect("identity exceeds the address space"),
            1 << (bit % 64),
        )
    }

    pub fn contains(&self, identity: Id) -> bool {
        let (word, mask) = Self::coordinate(identity);
        self.words.get(word).is_some_and(|bits| bits & mask != 0)
    }

    pub fn insert(&mut self, identity: Id) -> bool {
        let (word, mask) = Self::coordinate(identity);
        if word >= self.words.len() {
            self.words.resize(word + 1, 0);
        }
        let absent = self.words[word] & mask == 0;
        self.words[word] |= mask;
        absent
    }

    /// Adds every member of `other` using the existing dense bit storage.
    pub fn union_with(&mut self, other: &Self) {
        self.words
            .resize(self.words.len().max(other.words.len()), 0);
        for (word, incoming) in self.words.iter_mut().zip(&other.words) {
            *word |= incoming;
        }
    }

    pub fn remove(&mut self, identity: Id) -> bool {
        let (word, mask) = Self::coordinate(identity);
        let Some(bits) = self.words.get_mut(word) else {
            return false;
        };
        let present = *bits & mask != 0;
        *bits &= !mask;
        present
    }
}

impl<Id> Default for IdSet<Id> {
    fn default() -> Self {
        Self {
            words: Vec::new(),
            identity: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct TestId(std::num::NonZeroU64);
    impl NumericId for TestId {
        fn new(raw: u64) -> Option<Self> {
            std::num::NonZeroU64::new(raw).map(Self)
        }
        fn get(self) -> u64 {
            self.0.get()
        }
    }

    fn id(raw: u64) -> TestId {
        TestId::new(raw).unwrap()
    }

    #[test]
    fn dense_set_union_preserves_both_sides_across_word_boundaries() {
        let mut left = IdSet::new();
        left.insert(id(2));
        let mut right = IdSet::new();
        right.insert(id(65));
        left.union_with(&right);
        assert!(left.contains(id(2)));
        assert!(left.contains(id(65)));
        assert!(!left.contains(id(64)));
        assert!(!right.contains(id(2)));
        left.union_with(&IdSet::new());
        assert!(left.contains(id(65)));
    }

    #[test]
    fn table_holds_one_consecutive_run_in_identity_order() {
        let mut table = IdTable::from_rows([(id(4), 'a'), (id(5), 'b'), (id(6), 'c')]).unwrap();
        assert_eq!(table.get(id(3)), None);
        assert_eq!(table.get(id(5)), Some(&'b'));
        assert_eq!(table.get(id(7)), None);
        assert_eq!(table.keys().collect::<Vec<_>>(), [id(4), id(5), id(6)]);
        assert_eq!(table.identity_at(0), Some(id(4)));
        assert_eq!(table.identity_at(2), Some(id(6)));
        assert_eq!(table.identity_at(3), None);
        assert_eq!(table.identity_at(usize::MAX), None);
        assert_eq!(IdTable::<TestId, ()>::default().identity_at(0), None);
        let high = IdTable::from_rows([(id(u64::MAX), ())]).unwrap();
        assert_eq!(high.identity_at(0), Some(id(u64::MAX)));
        assert_eq!(high.identity_at(1), None);
        assert_eq!(
            table.map(|_, value| value.to_ascii_uppercase()).get(id(6)),
            Some(&'C')
        );
        for (identity, value) in table.iter_mut().rev() {
            if identity == id(5) {
                *value = 'B';
            }
        }
        assert_eq!(
            table
                .iter()
                .map(|(id, value)| (id, *value))
                .collect::<Vec<_>>(),
            [(id(4), 'a'), (id(5), 'B'), (id(6), 'c')]
        );
        assert_eq!(
            IdTable::from_rows([(id(1), ()), (id(3), ())]),
            Err(NonDenseIdentities {
                expected: 2,
                found: 3
            })
        );
        assert!(IdTable::from_rows([(id(2), ()), (id(1), ())]).is_err());
        assert!(IdTable::from_rows([(id(2), ()), (id(2), ())]).is_err());
    }

    #[test]
    fn subset_rows_keep_coordinates_and_refuse_invalid_or_excessive_spans() {
        let rows =
            IdTable::from_rows_with_gaps([(id(4), Some('a')), (id(7), Some('b'))], 4).unwrap();
        assert_eq!(rows.get(id(4)), Some(&Some('a')));
        assert_eq!(rows.get(id(5)), Some(&None));
        assert_eq!(rows.get(id(7)), Some(&Some('b')));
        assert_eq!(rows.get(id(8)), None);
        assert_eq!(
            rows.keys().collect::<Vec<_>>(),
            [id(4), id(5), id(6), id(7)]
        );
        let high = IdTable::from_rows_with_gaps([(id(u64::MAX), Some('z'))], 1).unwrap();
        assert_eq!(high.get(id(u64::MAX)), Some(&Some('z')));
        assert!(matches!(
            IdTable::from_rows_with_gaps([(id(4), Some('a'))], 0),
            Err(IdTableBuildError::SpanLimit {
                span: 1,
                maximum: 0
            })
        ));
        assert!(IdTable::from_rows([(id(4), ()), (id(7), ())]).is_err());
        assert!(IdTable::from_rows_with_gaps([(id(7), Some('b')), (id(7), Some('c'))], 4).is_err());
        assert!(IdTable::from_rows_with_gaps([(id(7), Some('b')), (id(4), Some('c'))], 4).is_err());
        assert!(matches!(
            IdTable::from_rows_with_gaps([(id(1), Some('a')), (id(u64::MAX), Some('b'))], 16),
            Err(IdTableBuildError::SpanLimit { .. }) | Err(IdTableBuildError::SpanOverflow)
        ));
    }

    #[test]
    fn set_tracks_membership_across_words() {
        let mut set = IdSet::new();
        assert!(set.insert(id(70)));
        assert!(set.insert(id(1)));
        assert!(!set.insert(id(70)));
        assert!(set.remove(id(70)) && !set.remove(id(70)) && !set.remove(id(900)));
    }
}
