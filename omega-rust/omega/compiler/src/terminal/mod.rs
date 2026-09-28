//! Compiler packaging from a checked compilation to the canonical Terminal artifact.
//!
//! Start at `terminal_artifact.rs`. This compiler operation consumes one sealed checked
//! compilation and produces one retained Terminal product with its callback
//! custody and native-realization proposal. `native_proposal` projects what
//! a later native realization may consume. Omega stage 00 owns boundary-application
//! and comparison settlement from authored occurrences to selected provider evidence.

mod native_proposal;
mod terminal_artifact;

pub(crate) use terminal_artifact::produce_retained_terminal_artifact;
pub use terminal_artifact::{
    produce_terminal_report, validate_lowered_ieee_float_comparison_custody,
    validate_lowered_integer_comparison_custody,
};
