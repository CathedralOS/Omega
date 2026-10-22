#![forbid(unsafe_code)]

//! Arena storage primitives for Psi-owned source representations.
//!
//! `Arena` builds one dense node-kind page linked by `Handle` and `HandleSpan`.
//! `PagedArenaSet` retains several independently built dense pages behind the
//! same direct handle surface, while `PagedArena` and `GenerationalPagedArena`
//! serve sequential hierarchy and reusable-slot ownership respectively.
//! `HierarchyArena` gives parents an owned child range, and `OrderedRootArena`
//! keeps root order stable.
//! `ConcurrentPagedArena` lets workers publish their final vector allocations
//! directly, then seals those pages for ordinary reads and exclusive later-stage
//! rewrites without flattening or building a lookup table.
//! Handle zero is the shared null entry, so absence needs no `Option`.
//!
//! Start at `arena.rs`; `handle` holds the handle and span types, and each
//! remaining folder owns one arena kind.

mod arena;
mod concurrent_paged_arena;
mod generational_paged_arena;
mod handle;
mod hierarchy_arena;
mod ordered_root_arena;
mod owned_index;
mod owned_interner;
mod paged_arena;
mod paged_arena_set;

pub use arena::{Arena, ArenaIter, ArenaSpanInserter};
pub use concurrent_paged_arena::{
    ConcurrentArenaPage, ConcurrentPagedArena, IncompletePagePublication, PublishedArenaPage,
    SealedPagedArena,
};
pub use generational_paged_arena::{GenerationalPagedArena, SlotRef};
pub use handle::handle_span::HandleSpan;
pub use handle::{Handle, unique_handles_in_order};
pub use hierarchy_arena::{
    HierarchyArena, HierarchyArenaBuilder, HierarchyChildHandles, HierarchyNode,
};
pub use ordered_root_arena::{OrderedRootArena, OrderedRootArenaIter};
pub use owned_index::OwnedIndex;
pub use owned_interner::OwnedInterner;
pub use paged_arena::{PagedArena, PagedArenaIter, PagedSlice, PagedSliceIter};
pub use paged_arena_set::{PagedArenaFrontier, PagedArenaSet};
