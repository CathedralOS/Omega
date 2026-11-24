//! We link compiler rows with typed handles. The two 32-bit coordinates name a
//! slot and its generation in the issuing arena, so the backing buffer can grow
//! without invalidating those links. A dense `Arena` starts with one generation
//! for all its rows. When we retire a row, we track its occupancy and advance
//! its generation. Reusing the slot then gives the new row a different handle.
//!
//! Slot zero names absence. `Arena::get` reads the shared default at zero and
//! rejects an unresolved nonzero handle. Use `Arena::try_get` when a handle may
//! be stale. A `GenerationalPagedArena` returns a marked dummy for every
//! unresolved handle. Check `SlotRef::is_dummy` when that distinction matters.
//!
//! `HierarchyArena` stores a parent's children as a contiguous handle span.
//! `OrderedRootArena` maintains a root order over its backing arena.
//! `RetainedArena` lets a continuation append rows while sharing published
//! pages. `RootedArena` lets different declaration rosters share record
//! batches, which `RetainedRows` can expose in logical order.
//!
//! We keep indexes beside the owners they summarize. An `OwnedIndex` reuses
//! its data when the supplied revision matches. An arena issues a fresh
//! revision after mutation or storage replacement. For an index over several
//! inputs, the caller supplies a key that accounts for every input it reads.
//! The unit tests in `arena.rs`, `paged.rs` and `owned_index.rs` cover handle
//! reuse, stale lookups and index refresh.
//!
//! Build an index from a live row, refresh it after editing the row, then
//! retire the row before reusing its slot.
//!
//! ```
//! use arena::{Arena, OwnedIndex};
//!
//! let mut rows = Arena::new();
//! let first = rows.insert(7u64);
//! let index = OwnedIndex::default();
//! let read_index = |rows: &Arena<u64>| {
//!     index.with(rows.revision(), || *rows.get(first) * 2, |doubled| *doubled)
//! };
//! assert_eq!(read_index(&rows), 14);
//!
//! *rows.get_mut(first) = 11;
//! assert_eq!(read_index(&rows), 22);
//!
//! assert!(rows.free(first));
//! let next = rows.insert(9);
//! assert_eq!(next.arena_index(), first.arena_index());
//! assert!(rows.try_get(first).is_none());
//! assert_eq!(*rows.get(next), 9);
//! ```

#![forbid(unsafe_code)]

mod arena;
mod index_map;
mod owned_index;
mod paged;
mod retained;
mod retained_rows;
mod rooted;
pub use retained::RetainedArena;
pub use retained_rows::{RetainedRows, RetainedRowsIter};
pub use rooted::{RootRows, RootRowsIter, RootedArena};

pub use arena::{
    Arena, ArenaIter, ArenaSlotStorage, ArenaSpanInserter, ArenaStorageView, Handle, HandleSpan,
    HierarchyArena, HierarchyNode, HierarchyStorage, OrderedRootArena, OrderedRootArenaIter,
    SpanHandles, unique_handles_in_order,
};
pub use index_map::{BuildIndexHasher, HandleKeyMap, IndexHasher};
pub use owned_index::*;
pub use paged::{
    ConcurrentArenaPage, ConcurrentPagedArena, GenerationalPagedArena, IncompletePagePublication,
    OwnedInterner, OwnedTextInterner, PagedArenaFrontier, PagedArenaSet, PublishedArenaPage,
    SealedPagedArena, SlotRef,
};

mod id_table;
pub use id_table::{IdSet, IdTable, IdTableBuildError, NonDenseIdentities, NumericId};

mod normalized_set;
pub use normalized_set::NormalizedIdentitySet;
