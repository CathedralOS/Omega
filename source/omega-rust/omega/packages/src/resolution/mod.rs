//! Turn checked `build.omg` declarations into an exact immutable package
//! closure and independently validated compiler inputs.
//!
//! [`source`] binds declared local, workspace, and Git locations to source
//! custody. [`graph`] follows those bindings, rejects conflicts, and gives the
//! complete closure a canonical review subject. [`package_compilation_inputs`]
//! revalidates that custody at the compiler handoff.

mod compiler_input;
pub mod graph;
pub mod source;

pub(crate) use compiler_input::reachable_package_keys;
pub use compiler_input::{package_compilation_inputs, package_compilation_inputs_for};
