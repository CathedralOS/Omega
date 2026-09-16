#![forbid(unsafe_code)]

//! Arena storage primitives for Psi-owned source representations.
//!
//! One contiguous `Arena` per node kind, linked by `Handle` and `HandleSpan`
//! rather than owned vectors. `PagedArena` and `GenerationalPagedArena` grow
//! without moving existing pages, `HierarchyArena` gives parents an owned child
//! range for scoped lookup, and `OrderedRootArena` keeps root order stable.
//! Handle zero is the shared null entry, so absence needs no `Option`.
//!
//! Start at `arena.rs`; `handle` holds the handle and span types, and each
//! remaining folder owns one arena kind.

mod arena;
mod generational_paged_arena;
mod handle;
mod hierarchy_arena;
mod ordered_root_arena;
mod paged_arena;

pub use arena::{Arena, ArenaIter, ArenaSpanInserter};
pub use generational_paged_arena::{GenerationalPagedArena, SlotRef};
pub use handle::Handle;
pub use handle::handle_span::HandleSpan;
pub use hierarchy_arena::{
    HierarchyArena, HierarchyArenaBuilder, HierarchyChildHandles, HierarchyNode,
};
pub use ordered_root_arena::{OrderedRootArena, OrderedRootArenaIter};
pub use paged_arena::{PagedArena, PagedArenaIter, PagedSlice, PagedSliceIter};
