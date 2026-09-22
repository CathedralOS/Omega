#![forbid(unsafe_code)]

//! Checked-source product orchestration through Terminal Psi publication.
//!
//! This coordinator sequences lowering, selected optimization and publication.
//! Checked-source receipts stay beside the portable artifact, not inside it.

mod checked_ledger;
mod stage_timings;
mod terminal_production;
pub use checked_trees_to_lowered_psi::TerminalMachineSelection;
pub use stage_timings::{TerminalProductionStage, TerminalProductionTimings};
pub use terminal_production::{
    CallbackCustodyTerminalArtifactProductionError, ProducedProgramEntryTerminalArtifact,
    ProducedTerminalArtifact, ProducedTerminalArtifactWithCallbackCustody,
    ProgramEntryTerminalReceiptError, TerminalArtifactProductionError, TerminalProductionRequest,
};
