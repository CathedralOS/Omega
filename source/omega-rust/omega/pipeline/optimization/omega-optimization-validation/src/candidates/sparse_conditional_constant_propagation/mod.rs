//! Independent SCCP candidate validation entrance.
//!
//! `candidate_validation` owns the public acceptance joins and observation
//! equality, `integer_evaluation` reconstructs exact arithmetic, and
//! `snapshot_reconstruction` rebuilds the SCCP lattice without producer state.

use super::*;

mod candidate_validation;
mod integer_evaluation;
mod snapshot_reconstruction;

#[cfg(test)]
pub(crate) use candidate_validation::{
    ValidatedIntegerRangeComparisonKind, ValidatedIntegerRangePairComparisonKind,
    independently_evaluate_integer_range_comparison,
    independently_evaluate_integer_range_pair_comparison,
    independently_validated_integer_range_comparison_kind,
    independently_validated_integer_range_pair_comparison_kind,
};
pub(crate) use candidate_validation::{observation_at, same_closed_scalar_observation};
pub use candidate_validation::{
    validate_boolean_evaluation_candidate, validate_integer_evaluation_candidate,
    validate_scalar_evaluation_candidate,
};
pub(crate) use integer_evaluation::literal_boolean_fact;
pub(crate) use snapshot_reconstruction::{
    scalar_value_definition, validator_scalar_constant_facts,
};
