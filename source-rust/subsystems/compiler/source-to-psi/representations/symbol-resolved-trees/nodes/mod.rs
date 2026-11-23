//! Declarations and their name-bound types, values, calls and control flow.
pub mod callable_values;
mod control_flow_and_calls;
mod declarations_and_machines;
pub mod evidence;
pub(crate) mod state_bodies;
mod type_system_and_names;
pub mod values;

pub use control_flow_and_calls::{calls, control_flow};
pub use declarations_and_machines::{declarations, machine_declarations};
pub use type_system_and_names::{names, type_system};
