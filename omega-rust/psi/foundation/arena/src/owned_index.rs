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
/// revision key the caller supplies. A single arena uses its `u64` revision;
/// multiple independently changing inputs need their exact tuple, not a sum
/// that a shrinking visible roster can cancel. Lock poisoning is treated as a
/// stale entry: the index is pure derived content, so a panicked builder
/// leaves at most a rebuildable slot.
pub struct OwnedIndex<T, Revision = u64> {
    slot: Mutex<Option<(Revision, T)>>,
}

impl<T, Revision> Default for OwnedIndex<T, Revision> {
    fn default() -> Self {
        Self {
            slot: Mutex::new(None),
        }
    }
}

// A cloned owner keeps its source's arena revisions, so the clone can start
// warm with the same derived content.
impl<T: Clone, Revision: Clone> Clone for OwnedIndex<T, Revision> {
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
    /// Run `run` against the index, building it with `build` when the stored
    /// entry was computed against a different `revision`.
    pub fn with<R>(
        &self,
        revision: Revision,
        build: impl FnOnce() -> T,
        run: impl FnOnce(&T) -> R,
    ) -> R {
        let mut slot = self.slot.lock().unwrap_or_else(|error| error.into_inner());
        if !slot.as_ref().is_some_and(|(seen, _)| seen == &revision) {
            *slot = Some((revision, build()));
        }
        run(&slot.as_ref().expect("index just built").1)
    }

    /// Run `run` against the index mutably — for indexes whose maps grow
    /// verdict entries as queries arrive. The stored revision still marks
    /// the table state the index was built for.
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

    /// Drop the stored index; the next query rebuilds.
    pub fn clear(&mut self) {
        *self
            .slot
            .get_mut()
            .unwrap_or_else(|error| error.into_inner()) = None;
    }
}
