#![forbid(unsafe_code)]

//! Target-neutral production of Terminal Psi from one checked machine.
//!
//! `TerminalProductionRequest::produce` owns the single sequence of existing
//! Psi stages 05-07, including source custody and optional entry receipts.
//! Product compilation and early closed compile-time jobs need this same
//! operation; placing it below the Omega driver avoids making Psi compilation
//! depend on product scheduling, target realization or host reporting.
//!
//! The caller must supply a checked, complete execution closure. This owner
//! does not check a partial application, evaluate constants, install imports or
//! execute guest code. It preserves the existing stage judgments; extracting
//! production is not permission to bypass checking or certificate admission.
//! Observation wraps each leg without owning its result or changing its order.
//! Omega adapts observations and optimizer refusals to its product report.

mod custody;
mod observation;
mod terminal_production;

pub use checked_trees_to_lowered_psi::TerminalMachineSelection;
pub use custody::{ProducedTerminalArtifact, TerminalProductionCustody};
pub use observation::{TerminalProductionObserver, TerminalProductionStage, UnmeasuredProduction};
pub use terminal_production::{
    ProgramEntryTerminalReceiptError, TerminalArtifactProductionError, TerminalProductionRequest,
};
