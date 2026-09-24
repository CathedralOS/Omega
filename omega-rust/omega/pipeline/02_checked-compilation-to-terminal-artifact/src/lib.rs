#![forbid(unsafe_code)]

//! Checked compilation to the canonical Terminal artifact.
//!
//! Start at `terminal_artifact.rs`. This stage consumes one sealed checked
//! compilation and produces the Terminal artifact: the retained product with
//! its callback custody and native-realization proposal, or the program-entry
//! artifact the direct native route realizes. `native_proposal` projects what
//! a later native realization may consume, `application_coverage` rejoins
//! boundary applications to their checked realizations,
//! `float_comparisons` rejoins IEEE comparisons to their selected meanings,
//! `integer_comparisons` rejoins integer comparisons the same way, and
//! `float_fma` rejoins selected nearest fused multiply-adds to their plans.

mod application_coverage;
mod float_comparisons;
mod float_fma;
mod integer_comparisons;
mod native_proposal;
mod terminal_artifact;

pub use terminal_artifact::{
    ProgramEntryTerminalArtifact, produce_program_entry_terminal_artifact, produce_terminal_report,
    validate_lowered_ieee_float_comparison_custody, validate_lowered_integer_comparison_custody,
};
