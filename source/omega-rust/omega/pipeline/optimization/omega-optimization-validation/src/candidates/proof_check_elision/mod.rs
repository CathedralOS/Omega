//! Optimizer module role: stage group. Proof-certified scalar rewrite validation families.
//!
//! `identity_classification` rebuilds the admissible exact identity,
//! `candidate_validation` owns the general acceptance join, and the
//! `same_operand` and `unit_divisor` leaves own their narrower integer laws.

use super::*;

mod candidate_validation;
mod identity_classification;
mod same_operand;
mod unit_divisor;

pub use candidate_validation::validate_proof_certified_scalar_identity_candidate;
pub use same_operand::{
    validate_proof_certified_exact_integer_self_subtract_candidate,
    validate_proof_certified_integer_self_divide_candidate,
    validate_proof_certified_integer_self_remainder_candidate,
};
pub use unit_divisor::{
    validate_proof_certified_integer_remainder_by_one_candidate,
    validate_proof_certified_signed_integer_remainder_by_negative_one_candidate,
};
