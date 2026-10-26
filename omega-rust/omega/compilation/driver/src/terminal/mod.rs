//! Compiler packaging from a checked compilation to the canonical Terminal artifact.
//!
//! Start at `terminal_artifact.rs`. This compiler operation consumes one sealed checked
//! compilation and produces one retained Terminal product with its callback
//! custody and native-realization proposal. `native_proposal` produces callbacks
//! and passes product-owned request projections to Omega stage 00. That stage
//! owns boundary-application and comparison settlement from authored occurrences
//! to selected provider evidence.

mod behavior_exclusions;
mod native_proposal;
mod terminal_artifact;

pub(crate) use terminal_artifact::{
    ProducedTerminalApplication, TerminalProductRequest, TerminalProductionView,
    finish_terminal_application, produce_retained_terminal_artifacts,
};
