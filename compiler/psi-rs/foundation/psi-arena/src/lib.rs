#![forbid(unsafe_code)]

//! Arena storage primitives for Psi-owned source representations.

mod arena;
mod handle;
mod handle_span;

pub use arena::{Arena, ArenaIter, ArenaSpanInserter};
pub use handle::Handle;
pub use handle_span::HandleSpan;
