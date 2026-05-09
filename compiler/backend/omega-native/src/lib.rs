//! Native backend implementation.

pub mod data;
pub mod emission;
pub mod identity;
pub mod instructions;
pub mod plan;
pub mod report;
pub mod runtime_dispatch;
pub mod runtime_flow;
pub mod runtime_storage;
pub mod runtime_text;
pub mod state_analysis;
pub mod state_schedule;
pub mod state_values;

pub use runtime_dispatch::guards as state_guards;
pub use runtime_dispatch::states as state_dispatch;
