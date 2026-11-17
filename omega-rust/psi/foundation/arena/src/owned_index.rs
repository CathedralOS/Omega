/*
We implement the revision-keyed lazy index owned directly by the arena-bearing value
it summarizes.

`Arena::revision()` is a process-unique stamp of one storage at one mutation epoch: a
clone keeps it, and any mutation-capable call or replacement storage yields a stamp never
seen before. A tuple of the stamps and complete rosters an index reads is therefore its
exact freshness key; a sum or a lossy roster marker is not. Pairing
the cached value with the revision it was built against inside the owning struct ensures
a dropped owner takes its index with it and a mutated owner rebuilds on the next query.
Composite arenas keep their own stamp: root selection, page membership and mutable row
access invalidate that owner's epoch. Reading it does not scan or add child epochs.
When an owner finishes mutating, `seal` moves the built index out of the mutex slot so
parallel workers read the frozen copy without locking.

Clones share the immutable index data, not the mutex or its revision slot.
All index updates must use `with_mut` or `seal_with`; those detach shared data
before handing out mutable access. Read callbacks must not mutate the index
through interior mutability.
*/

use std::sync::{Arc, Mutex};

pub struct OwnedIndex<T, Revision = u64> {
    slot: Mutex<Option<(Revision, Arc<T>)>>, // Lazy revision-tagged index slot for mutable owners.
    sealed: Option<(Revision, Arc<T>)>,      // Lock-free frozen index for sealed owners.
}

pub type RevisionIndex<T, Revision = u64> = OwnedIndex<T, Revision>;

impl<T, Revision> Default for OwnedIndex<T, Revision> {
    fn default() -> Self {
        Self {
            slot: Mutex::new(None),
            sealed: None,
        }
    }
}

// A continuation initially reads the same immutable index. Its mutex and
// revision remain private; rebuilding replaces only its slot, and a mutation
// copies the index before exposing it to the caller.
impl<T, Revision: Clone> Clone for OwnedIndex<T, Revision> {
    fn clone(&self) -> Self {
        Self {
            slot: Mutex::new(
                self.slot
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .clone(),
            ),
            sealed: self.sealed.clone(),
        }
    }
}

impl<T, Revision> PartialEq for OwnedIndex<T, Revision> {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl<T, Revision> Eq for OwnedIndex<T, Revision> {}

impl<T, Revision> std::fmt::Debug for OwnedIndex<T, Revision> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("OwnedIndex")
    }
}

impl<T, Revision: PartialEq> OwnedIndex<T, Revision> {
    pub fn with<R>(
        &self,
        revision: Revision,
        build: impl FnOnce() -> T,
        run: impl FnOnce(&T) -> R,
    ) -> R {
        if let Some((sealed, index)) = &self.sealed
            && *sealed == revision
        {
            return run(index);
        }
        let mut slot = self.slot.lock().unwrap_or_else(|error| error.into_inner());
        if !slot.as_ref().is_some_and(|(seen, _)| seen == &revision) {
            *slot = Some((revision, Arc::new(build())));
        }
        run(&slot.as_ref().expect("index just built").1)
    }

    pub fn seal(&mut self) {
        if let Some(built) = self
            .slot
            .get_mut()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        {
            self.sealed = Some(built);
        }
    }

    pub fn seal_with(&mut self, finish: impl FnOnce(&mut T))
    where
        T: Clone,
    {
        self.seal();
        if let Some((_, index)) = &mut self.sealed {
            finish(Arc::make_mut(index));
        }
    }

    pub fn unseal(&mut self) {
        if let Some(sealed) = self.sealed.take() {
            *self
                .slot
                .get_mut()
                .unwrap_or_else(|error| error.into_inner()) = Some(sealed);
        }
    }

    pub fn with_mut<R>(
        &self,
        revision: Revision,
        build: impl FnOnce() -> T,
        run: impl FnOnce(&mut T) -> R,
    ) -> R
    where
        T: Clone,
    {
        let mut slot = self.slot.lock().unwrap_or_else(|error| error.into_inner());
        if !slot.as_ref().is_some_and(|(seen, _)| seen == &revision) {
            *slot = Some((revision, Arc::new(build())));
        }
        run(Arc::make_mut(
            &mut slot.as_mut().expect("index just built").1,
        ))
    }

    pub fn clear(&mut self) {
        self.sealed = None;
        *self
            .slot
            .get_mut()
            .unwrap_or_else(|error| error.into_inner()) = None;
    }
}

impl<T, Revision> compiler_storage::Record for OwnedIndex<T, Revision> {
    fn write(
        &self,
        _: &mut compiler_storage::RecordWriter,
    ) -> Result<(), compiler_storage::RecordError> {
        Ok(())
    }
    fn read<Context>(
        _: &mut compiler_storage::RecordReader<'_, Context>,
    ) -> Result<Self, compiler_storage::RecordError> {
        Ok(Self::default())
    }
}

#[cfg(test)]
mod tests {
    use super::OwnedIndex;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountedRows {
        rows: Vec<u32>,
        copies: Arc<AtomicUsize>,
    }

    impl Clone for CountedRows {
        fn clone(&self) -> Self {
            self.copies.fetch_add(1, Ordering::Relaxed);
            Self {
                rows: self.rows.clone(),
                copies: Arc::clone(&self.copies),
            }
        }
    }

    fn populated() -> (OwnedIndex<CountedRows>, Arc<AtomicUsize>) {
        let copies = Arc::new(AtomicUsize::new(0));
        let index = OwnedIndex::default();
        index.with(
            7,
            || CountedRows {
                rows: vec![11, 22],
                copies: Arc::clone(&copies),
            },
            |_| (),
        );
        (index, copies)
    }

    fn values(index: &OwnedIndex<CountedRows>, revision: u64) -> Vec<u32> {
        index.with(
            revision,
            || panic!("the retained index must be used"),
            |data| data.rows.clone(),
        )
    }

    #[test]
    fn forks_share_index_data_until_their_first_write() {
        for sealed in [false, true] {
            let (mut original, copies) = populated();
            if sealed {
                original.seal();
            }
            let mut fork = original.clone();
            assert_eq!(copies.load(Ordering::Relaxed), 0);
            let pointer = |data: &CountedRows| data.rows.as_ptr();
            assert_eq!(
                original.with(7, || panic!("retained original"), pointer),
                fork.with(7, || panic!("retained fork"), pointer)
            );
            fork.unseal();
            fork.with_mut(7, || panic!("retained fork"), |data| data.rows[0] = 33);
            assert_eq!(copies.load(Ordering::Relaxed), 1);
            assert_eq!(values(&original, 7), [11, 22]);
            assert_eq!(values(&fork, 7), [33, 22]);
            fork.with_mut(7, || panic!("retained fork"), |data| data.rows.push(44));
            assert_eq!(copies.load(Ordering::Relaxed), 1);
            assert_eq!(values(&fork, 7), [33, 22, 44]);
            assert_eq!(values(&original, 7), [11, 22]);
        }
    }

    #[test]
    fn finishing_a_shared_sealed_index_does_not_modify_its_sibling() {
        let (mut original, copies) = populated();
        original.seal();
        let mut fork = original.clone();
        fork.seal_with(|data| data.rows.reverse());
        assert_eq!(copies.load(Ordering::Relaxed), 1);
        assert_eq!(values(&original, 7), [11, 22]);
        assert_eq!(values(&fork, 7), [22, 11]);
        fork.seal_with(|data| data.rows.push(55));
        assert_eq!(copies.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn changing_revision_or_clearing_a_fork_keeps_the_original_index() {
        let (original, copies) = populated();
        let mut fork = original.clone();
        fork.with(
            8,
            || CountedRows {
                rows: vec![99],
                copies: Arc::clone(&copies),
            },
            |_| (),
        );
        assert_eq!(copies.load(Ordering::Relaxed), 0);
        assert_eq!(values(&original, 7), [11, 22]);
        assert_eq!(values(&fork, 8), [99]);
        fork.clear();
        assert_eq!(values(&original, 7), [11, 22]);
        assert_eq!(
            fork.with(
                8,
                || CountedRows {
                    rows: vec![66],
                    copies
                },
                |data| data.rows[0]
            ),
            66
        );
    }
}
