//! Optimizer module role: stage group. Independent candidate acceptance, organized by the producing pass family.

mod case_membership_specialization;
mod control_flow_cleanup;
mod copy_propagation;
mod dead_scalar_elimination;
mod dispatch;
mod field_value_specialization;
mod global_value_numbering;
mod observation;
mod proof_check_elision;
mod rewrite_accounting;
mod sparse_conditional_constant_propagation;
mod state_specialization;
mod structural_bindings;

pub use case_membership_specialization::*;
pub use control_flow_cleanup::*;
pub use copy_propagation::*;
pub use dead_scalar_elimination::*;
pub use dispatch::*;
pub use field_value_specialization::*;
pub use global_value_numbering::*;
pub use observation::*;
pub use proof_check_elision::*;
pub use sparse_conditional_constant_propagation::*;
pub use state_specialization::*;

pub(crate) use copy_propagation::rewrite_block_parameter_operation;
pub(crate) use global_value_numbering::{
    independent_reachable_dominators, independently_accepted_operation_fact,
    independently_replacement_dominates_uses,
};
pub(crate) use rewrite_accounting::*;
pub(crate) use sparse_conditional_constant_propagation::{
    literal_boolean_fact, observation_at, same_closed_scalar_observation, scalar_value_definition,
    validator_scalar_constant_facts,
};
