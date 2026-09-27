//! An index owned by the arena-bearing value it describes.
//!
//! `Arena::revision()` moves on every mutation-capable call, so the revision
//! is the exact freshness key for anything derived from an arena's contents.
//! `OwnedIndex` pairs a lazily built index with the revision it was computed
//! against; a query rebuilds when the owner's revision moved. Because the
//! slot lives inside the owner, no caller-side identity key is needed — a
//! dropped owner takes its index with it, and a replacement can never
//! inherit verdicts it did not earn.

use std::sync::Mutex;

/// A lazily built index owned by the value it describes, guarded by the
/// `u64` revision the caller supplies (an arena's own revision, or a sum of
/// every arena revision the index reads). Lock poisoning is treated as a
/// stale entry: the index is pure derived content, so a panicked builder
/// leaves at most a rebuildable slot.
pub struct OwnedIndex<T> {
    slot: Mutex<Option<(u64, T)>>,
}

impl<T> Default for OwnedIndex<T> {
    fn default() -> Self {
        Self {
            slot: Mutex::new(None),
        }
    }
}

// A cloned owner keeps its source's arena revisions, so the clone can start
// warm with the same derived content.
impl<T: Clone> Clone for OwnedIndex<T> {
    fn clone(&self) -> Self {
        Self {
            slot: Mutex::new(
                self.slot
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .clone(),
            ),
        }
    }
}

// The index is derived content: two owners with equal tables are equal
// whether or not the index has been built yet.
impl<T> PartialEq for OwnedIndex<T> {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl<T> Eq for OwnedIndex<T> {}

impl<T> std::fmt::Debug for OwnedIndex<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("OwnedIndex")
    }
}

impl<T> OwnedIndex<T> {
    /// Run `run` against the index, building it with `build` when the stored
    /// entry was computed against a different `revision`.
    pub fn with<R>(
        &self,
        revision: u64,
        build: impl FnOnce() -> T,
        run: impl FnOnce(&T) -> R,
    ) -> R {
        let mut slot = self.slot.lock().unwrap_or_else(|error| error.into_inner());
        if !slot.as_ref().is_some_and(|(seen, _)| *seen == revision) {
            *slot = Some((revision, build()));
        }
        run(&slot.as_ref().expect("index just built").1)
    }

    /// Run `run` against the index mutably — for indexes whose maps grow
    /// verdict entries as queries arrive. The stored revision still marks
    /// the table state the index was built for.
    pub fn with_mut<R>(
        &self,
        revision: u64,
        build: impl FnOnce() -> T,
        run: impl FnOnce(&mut T) -> R,
    ) -> R {
        let mut slot = self.slot.lock().unwrap_or_else(|error| error.into_inner());
        if !slot.as_ref().is_some_and(|(seen, _)| *seen == revision) {
            *slot = Some((revision, build()));
        }
        run(&mut slot.as_mut().expect("index just built").1)
    }

    /// Drop the stored index; the next query rebuilds.
    pub fn clear(&mut self) {
        *self
            .slot
            .get_mut()
            .unwrap_or_else(|error| error.into_inner()) = None;
    }
}
