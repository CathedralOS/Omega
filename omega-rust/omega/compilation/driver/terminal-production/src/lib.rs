#![forbid(unsafe_code)]

//! Product reporting and custody around Psi-owned Terminal production.
//!
//! `TerminalProductionRequest::produce` adapts product timings and refusal
//! reporting to `psi_terminal_production`; it contains no lowering sequence.
//! Early compile-time jobs can depend on that Psi owner without importing this
//! Omega driver. Checked-source custody stays beside the portable artifact.

mod custody;
mod stage_timings;
mod terminal_production;
pub use custody::{ProducedTerminalArtifact, TerminalProductionCustody};
pub use psi_terminal_production::{TerminalMachineSelection, TerminalProductionStage};
pub use terminal_production::{
    ProgramEntryTerminalReceiptError, TerminalArtifactProductionError, TerminalProductionRequest,
};
