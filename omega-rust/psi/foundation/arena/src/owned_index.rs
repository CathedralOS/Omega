/*
We implement the revision-keyed lazy index owned directly by the arena-bearing value
it summarizes.

`Arena::revision()` advances on every mutation-capable call, making the revision tuple
the exact freshness key for any derived lookup table over an arena's contents. Pairing
the cached value with the revision it was built against inside the owning struct ensures
a dropped owner takes its index with it and a mutated owner rebuilds on the next query.
When an owner finishes mutating, `seal` moves the built index out of the mutex slot so
parallel workers read the frozen copy without locking.
*/

use std::sync::Mutex;

pub struct OwnedIndex<T, Revision = u64> {
    slot: Mutex<Option<(Revision, T)>>, // Lazy revision-tagged index slot for mutable owners.
    sealed: Option<(Revision, T)>,      // Lock-free frozen index for sealed owners.
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

impl<T: Clone, Revision: Clone> Clone for OwnedIndex<T, Revision> {
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
            *slot = Some((revision, build()));
        }
        run(&slot.as_ref().expect("index just built").1)
    }

    pub fn with_updated<R>(
        &self,
        revision: Revision,
        update: impl FnOnce(Option<&mut T>) -> Option<T>,
        run: impl FnOnce(&T) -> R,
    ) -> R {
        if let Some((sealed, index)) = &self.sealed
            && *sealed == revision
        {
            return run(index);
        }
        let mut slot = self.slot.lock().unwrap_or_else(|error| error.into_inner());
        match slot.as_mut() {
            Some((seen, existing)) if *seen == revision => run(existing),
            Some((seen, existing)) => {
                if let Some(rebuilt) = update(Some(existing)) {
                    *existing = rebuilt;
                }
                *seen = revision;
                run(existing)
            }
            None => {
                let built = update(None).expect("fresh index build must return a value");
                *slot = Some((revision, built));
                run(&slot.as_ref().expect("index just built").1)
            }
        }
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

    pub fn seal_with(&mut self, finish: impl FnOnce(&mut T)) {
        self.seal();
        if let Some((_, index)) = &mut self.sealed {
            finish(index);
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
    ) -> R {
        let mut slot = self.slot.lock().unwrap_or_else(|error| error.into_inner());
        if !slot.as_ref().is_some_and(|(seen, _)| seen == &revision) {
            *slot = Some((revision, build()));
        }
        run(&mut slot.as_mut().expect("index just built").1)
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
