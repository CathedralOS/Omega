//! Native backend implementation.

pub mod alias_flow;
pub mod data;
pub mod emission;
pub mod host_calls;
pub mod identity;
pub mod instructions;
pub mod object;
pub(crate) mod place_keys;
pub mod plan;
pub mod report;
pub mod runtime_dispatch;
pub mod runtime_flow;
pub mod runtime_storage;
pub mod runtime_text;
pub mod state_analysis;
pub mod state_calls;
pub mod state_schedule;
pub mod state_storage;
pub mod state_values;
pub(crate) mod storage_regions;

pub use runtime_dispatch::guards as state_guards;
pub use runtime_dispatch::states as state_dispatch;
