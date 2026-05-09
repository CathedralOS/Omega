//! Native backend implementation.

pub mod alias_flow;
pub mod architecture;
pub mod data;
pub mod emission;
pub mod emitter;
pub mod host_calls;
pub mod identity;
pub mod instructions;
pub mod machine_code;
pub mod object;
pub(crate) mod place_keys;
pub mod plan;
pub mod relocations;
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
pub mod target_output;

pub use runtime_dispatch::guards as state_guards;
pub use runtime_dispatch::states as state_dispatch;
