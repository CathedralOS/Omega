//! Sparse conditional constant propagation rule-family entrance.
//!
//! `constant_evaluation` owns exact constant-fold proposals and their rule
//! contracts. `range_comparisons` owns comparisons decided from closed integer
//! ranges. The parent pass catalog selects the family; this module publishes
//! its named rules.

mod catalog;

pub(in crate::rules) use catalog::built_in_registrations;

mod constant_evaluation;
pub(in crate::rules::passes) mod range_comparisons;

pub use constant_evaluation::*;
pub use range_comparisons::{
    IntegerEqualConstantRangeRule, IntegerEqualRangeConstantRule, IntegerEqualRangeRangeRule,
    IntegerLessOrEqualConstantRangeRule, IntegerLessOrEqualRangeConstantRule,
    IntegerLessOrEqualRangeRangeRule, IntegerLessThanConstantRangeRule,
    IntegerLessThanRangeConstantRule, IntegerLessThanRangeRangeRule,
};
