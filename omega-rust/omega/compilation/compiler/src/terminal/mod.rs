//! Attach each target's entry and execution choices to the shared program.
//!
//! `calling_program.rs` saves calling declarations and publishes portable code
//! once. `terminal_artifact.rs` reuses that publication for ordinary target
//! applications; named inspection has an explicit source-producing entrance.
//! Native application preparation checks the retained target choices and
//! produces a realization proposal without reopening source.

mod behavior_exclusions;
mod calling_program;
mod product_code;
mod terminal_artifact;
pub(crate) use calling_program::retain_calling_program;

pub(crate) use terminal_artifact::{
    ProducedTerminalProduct, TerminalProductRequest, TerminalProductUse, TerminalProductionView,
    check_terminal_conditions, produce_retained_terminal_artifacts,
};
