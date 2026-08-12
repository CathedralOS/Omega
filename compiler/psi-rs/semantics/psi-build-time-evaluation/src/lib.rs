#![forbid(unsafe_code)]

//! Target-neutral admission and execution of compile-time Omega machines.

mod admission;
mod const_generic_calls;
mod const_lengths;

pub use admission::BuildTimeAdmissionPlan;
pub use const_generic_calls::evaluate_const_generic_calls;
pub use const_lengths::{evaluate_const_array_lengths, evaluate_zero_argument_machine};
