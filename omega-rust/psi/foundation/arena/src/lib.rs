/*
We provide the dense arena, handle, paged-arena, and revision-indexed storage primitives
for Psi-owned compiler representations.

Our storage layer is organized into three modules:
- `arena`: `Handle`, `HandleSpan`, `SpanHandles`, `unique_handles_in_order`, the contiguous
  node-kind `Arena` (`ArenaIter`, `ArenaSlotStorage`, `ArenaSpanInserter`, `ArenaStorageView`),
  `HierarchyArena` (`HierarchyNode`), and `OrderedRootArena` (`OrderedRootArenaIter`).
- `paged`: multi-page and concurrent arena structures (`PagedArenaSet`, `PagedArenaFrontier`,
  `ConcurrentPagedArena`, `ConcurrentArenaPage`, `PublishedArenaPage`, `SealedPagedArena`,
  `IncompletePagePublication`, `GenerationalPagedArena`, `SlotRef`, `OwnedInterner`, and
  `OwnedTextInterner`).
- `owned_index`: the revision-guarded lazy derived index owned by the arena-bearing
  table it summarizes.
*/

/*
Compiler representations link syntax, symbol, type, proof, and flow nodes through dense
32-bit `(arena_index, generation)` coordinates rather than heap pointers or hash tables.
Handle zero is the shared null entry so absence requires no `Option` wrapper, and
append-only pages keep uniform slot identity without allocating per-row generation or
occupancy vectors until the first retirement occurs. Grouping contiguous single-page
arenas in `arena.rs`, paged and concurrent worker arenas in `paged.rs`, and revision-keyed
derived indexes in `owned_index.rs` keeps every storage primitive discoverable behind a
flat module surface.
*/

/*
We consolidated the seven subdirectories (`arena/`, `handle/`, `hierarchy_arena/`,
`ordered_root_arena/`, `paged_arena_set/`, `concurrent_paged_arena/`, and
`generational_paged_arena/`) and `owned_interner.rs` into `arena.rs` and `paged.rs`,
retaining `owned_index.rs` alongside `lib.rs`.
*/

#![forbid(unsafe_code)]

mod arena;
mod owned_index;
mod paged;

pub use arena::{
    Arena, ArenaIter, ArenaSlotStorage, ArenaSpanInserter, ArenaStorageView, Handle, HandleSpan,
    HierarchyArena, HierarchyNode, OrderedRootArena, OrderedRootArenaIter, SpanHandles,
    unique_handles_in_order,
};
pub use owned_index::*;
pub use paged::{
    ConcurrentArenaPage, ConcurrentPagedArena, GenerationalPagedArena, IncompletePagePublication,
    OwnedInterner, OwnedTextInterner, PagedArenaFrontier, PagedArenaSet, PublishedArenaPage,
    SealedPagedArena, SlotRef,
};
