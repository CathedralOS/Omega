//! Local, dominating, and phi-translated scalar CSE validation.
//!
//! `expression_keys` independently classifies scalar expressions,
//! `dominance_reconstruction` rebuilds the reachability proof, and
//! `candidate_validation` owns the three public acceptance joins.

use super::*;

mod candidate_validation;
mod dominance_reconstruction;
mod expression_keys;

pub(crate) use candidate_validation::independently_accepted_operation_fact;
pub use candidate_validation::{
    validate_dominating_scalar_common_subexpression_candidate,
    validate_local_scalar_common_subexpression_candidate,
    validate_phi_translated_scalar_common_subexpression_candidate,
};
pub(crate) use dominance_reconstruction::{
    independent_reachable_dominators, independently_replacement_dominates_uses,
};
