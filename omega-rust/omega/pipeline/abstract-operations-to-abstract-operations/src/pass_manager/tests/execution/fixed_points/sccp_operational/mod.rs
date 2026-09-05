//! Optimizer module role: stage group. Whole-engine operational custody for all exact SCCP roster rows.
//!
//! The three semantic families own their isolated fixtures. [`custody`] owns
//! the common disabled, deterministic, budget, manifest, ledger, and
//! fixed-point assertions.

mod boolean_constants;
mod custody;
mod integer_constants;
mod range_comparisons;
