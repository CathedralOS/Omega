#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Post-allocation machine analysis components.
//!
//! Start at `post_allocation_machine.rs`: the allocation phase supplies one
//! replayed current-program view, machine analysis consumes only current
//! facts, and the sealed plan retains allocation evidence separately. The
//! `plan` folder owns deterministic plan construction and replay. This stage
//! does not inspect rewrite history or select a different construction route.

mod plan;
mod post_allocation_machine;

pub use plan::*;
pub use post_allocation_machine::*;
