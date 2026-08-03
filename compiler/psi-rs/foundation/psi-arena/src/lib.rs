#![forbid(unsafe_code)]

//! Arena storage primitives for Psi-owned source representations.

mod arena;
mod free_stack;
mod generational_paged_arena;
mod handle;
mod handle_span;
mod hierarchy_arena;
mod ordered_root_arena;
mod paged_arena;

pub use arena::{Arena, ArenaIter, ArenaSpanInserter};
pub use generational_paged_arena::{GenerationalPagedArena, SlotRef};
pub use handle::Handle;
pub use handle_span::HandleSpan;
pub use hierarchy_arena::{
    HierarchyArena, HierarchyArenaBuilder, HierarchyChildHandles, HierarchyNode,
};
pub use ordered_root_arena::{OrderedRootArena, OrderedRootArenaIter};
pub use paged_arena::{PagedArena, PagedArenaIter, PagedSlice, PagedSliceIter};
