#![forbid(unsafe_code)]

//! Target-neutral admission and execution of compile-time Omega machines.

mod access_plans;
mod admission;
mod const_domain_facts;
mod const_generic_calls;
mod const_lengths;
mod layout_plans;
mod wire_plans;

pub use access_plans::{compute_access_plan, compute_placement_plan};
pub use admission::BuildTimeAdmissionPlan;
pub use const_domain_facts::evaluate_const_domain_facts;
pub use const_generic_calls::evaluate_const_generic_calls;
pub use const_lengths::{evaluate_const_array_lengths, evaluate_zero_argument_machine};
pub use layout_plans::{compute_layout_plan, normalized_schema_identity};
pub use wire_plans::compute_wire_plans;
