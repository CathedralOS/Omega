//! Native backend implementation.

pub mod identity;
pub mod instructions;
pub mod plan;
pub mod runtime_dispatch;
pub mod runtime_storage;
pub mod state_schedule;

pub use runtime_dispatch::guards as state_guards;
pub use runtime_dispatch::states as state_dispatch;
