//! Share unchanged scalar slots between retained execution states.
//!
//! The existing IdTable owns the original identity run and its gap validation.
//! Copies keep that immutable namespace and every slot, including holes and
//! unevaluated values. Updating one slot copies only its small shared page;
//! neither allocation sharing nor a missing value changes semantic identity.

use std::cmp::Ordering;
use std::sync::Arc;

use semantic_vocabulary::{IdTable, ValueId};

const PAGE_LEN: usize = 64;

#[derive(Clone, Debug)]
pub(super) struct HandleScalarTable<T> {
    namespace: Arc<IdTable<ValueId, ()>>,
    pages: Vec<Arc<Vec<T>>>,
}

impl<T> Default for HandleScalarTable<T> {
    fn default() -> Self {
        Self {
            namespace: Arc::new(IdTable::default()),
            pages: Vec::new(),
        }
    }
}

impl<T> HandleScalarTable<T> {
    fn position(&self, value: ValueId) -> Option<usize> {
        self.namespace.get(value)?;
        let first = self.namespace.keys().next()?;
        usize::try_from(value.get().checked_sub(first.get())?).ok()
    }

    pub(super) fn get(&self, value: ValueId) -> Option<&T> {
        let position = self.position(value)?;
        self.pages
            .get(position / PAGE_LEN)?
            .get(position % PAGE_LEN)
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = (ValueId, &T)> {
        self.namespace
            .keys()
            .zip(self.pages.iter().flat_map(|page| page.iter()))
    }
}

impl<T: Clone> HandleScalarTable<T> {
    /// The caller supplies its already validated, bounded original namespace.
    pub(super) fn from_table(table: IdTable<ValueId, T>) -> Self {
        let namespace = Arc::new(table.map(|_, _| ()));
        let mut pages = Vec::with_capacity(table.len().div_ceil(PAGE_LEN));
        let mut values = table.values().cloned();
        loop {
            let page = values.by_ref().take(PAGE_LEN).collect::<Vec<_>>();
            if page.is_empty() {
                break;
            }
            pages.push(Arc::new(page));
        }
        Self { namespace, pages }
    }

    pub(super) fn get_mut(&mut self, value: ValueId) -> Option<&mut T> {
        let position = self.position(value)?;
        Arc::make_mut(self.pages.get_mut(position / PAGE_LEN)?).get_mut(position % PAGE_LEN)
    }

    #[cfg(test)]
    pub(super) fn map(&self, mut derive: impl FnMut(ValueId, &T) -> T) -> Self {
        Self::from_table(
            IdTable::from_rows(self.iter().map(|(id, value)| (id, derive(id, value))))
                .expect("shared scalar pages retain their original ordered namespace"),
        )
    }
}

impl<T: Ord> Ord for HandleScalarTable<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.namespace.cmp(&other.namespace).then_with(|| {
            self.pages
                .iter()
                .zip(&other.pages)
                .map(|(left, right)| {
                    if Arc::ptr_eq(left, right) {
                        Ordering::Equal
                    } else {
                        left.cmp(right)
                    }
                })
                .find(|order| *order != Ordering::Equal)
                .unwrap_or_else(|| self.pages.len().cmp(&other.pages.len()))
        })
    }
}

impl<T: Ord> PartialOrd for HandleScalarTable<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Ord> PartialEq for HandleScalarTable<T> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<T: Ord> Eq for HandleScalarTable<T> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_handle_replay_snapshots_keep_original_high_ids_holes_and_isolate_changed_pages() {
        let first = ValueId::new(u64::MAX - 129).unwrap();
        let last = ValueId::new(u64::MAX).unwrap();
        let original =
            IdTable::from_rows_with_gaps([(first, Some(Some(7u64))), (last, Some(None))], 130)
                .unwrap();
        let expected = original
            .iter()
            .map(|(id, value)| (id, *value))
            .collect::<Vec<_>>();
        let baseline = HandleScalarTable::from_table(original);
        assert_eq!(
            baseline
                .iter()
                .map(|(id, value)| (id, *value))
                .collect::<Vec<_>>(),
            expected
        );
        let hole = ValueId::new(first.get() + 1).unwrap();
        assert_eq!(baseline.get(hole), Some(&None));
        assert_eq!(baseline.get(last), Some(&Some(None)));
        assert_eq!(baseline.get(ValueId::new(first.get() - 1).unwrap()), None);
        let mut next = baseline.clone();
        assert!(
            next.get_mut(ValueId::new(first.get() - 1).unwrap())
                .is_none()
        );
        assert!(Arc::ptr_eq(&baseline.namespace, &next.namespace));
        assert!(
            baseline
                .pages
                .iter()
                .zip(&next.pages)
                .all(|(a, b)| Arc::ptr_eq(a, b))
        );
        *next.get_mut(first).unwrap() = Some(Some(9));
        assert!(!Arc::ptr_eq(&baseline.pages[0], &next.pages[0]));
        assert!(
            baseline.pages[1..]
                .iter()
                .zip(&next.pages[1..])
                .all(|(a, b)| Arc::ptr_eq(a, b))
        );
        assert_eq!(baseline.get(first), Some(&Some(Some(7))));
        assert_eq!(next.get(first), Some(&Some(Some(9))));
        assert_ne!(baseline, next);
        *next.get_mut(first).unwrap() = Some(Some(7));
        assert_eq!(baseline, next, "sharing does not participate in equality");
        let singleton =
            HandleScalarTable::from_table(IdTable::from_rows([(last, Some(3u64))]).unwrap());
        assert_eq!(singleton.get(last), Some(&Some(3)));
        assert_eq!(singleton.pages.len(), 1);
        assert!(
            IdTable::<ValueId, Option<u64>>::from_rows_with_gaps(
                [(ValueId::new(1).unwrap(), None), (last, None)],
                130,
            )
            .is_err()
        );
    }
}
