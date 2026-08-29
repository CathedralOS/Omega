//! Independent candidate acceptance, organized by the producing pass family.

use super::*;

mod control_flow_cleanup;
mod copy_propagation;
mod dead_scalar_elimination;
mod dispatch;
mod global_value_numbering;
mod observation;
mod proof_check_elision;
mod rewrite_accounting;
mod sparse_conditional_constant_propagation;

pub use control_flow_cleanup::*;
pub use copy_propagation::*;
pub use dead_scalar_elimination::*;
pub use dispatch::*;
pub use global_value_numbering::*;
pub use observation::*;
pub use proof_check_elision::*;
pub use sparse_conditional_constant_propagation::*;

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
