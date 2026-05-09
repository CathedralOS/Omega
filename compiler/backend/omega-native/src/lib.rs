//! Native backend implementation.

pub mod instructions;
pub mod plan;
pub mod runtime_dispatch;
pub mod runtime_storage;

pub use runtime_dispatch::guards as state_guards;
