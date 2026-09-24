#![forbid(unsafe_code)]

//! Checked trees to a published Terminal Psi artifact.
//!
//! The one operation is `TerminalProductionRequest::produce`
//! (`terminal_production.rs`). It lowers the selected machine
//! (05_checked-trees-to-lowered-psi), runs the selected Psi optimization, and
//! publishes the canonical artifact (07_lowered-psi-to-terminal-psi).
//! Checked-source receipts stay beside the portable artifact, not inside it.

mod checked_ledger;
mod stage_timings;
mod terminal_production;
pub use checked_trees_to_lowered_psi::TerminalMachineSelection;
pub use stage_timings::{TerminalProductionStage, TerminalProductionTimings};
pub use terminal_production::{
    CallbackCustodyTerminalArtifactProductionError, ProducedTerminalArtifact,
    ProgramEntryTerminalReceiptError, TerminalArtifactProductionError, TerminalProductionCustody,
    TerminalProductionRequest,
};
