#![forbid(unsafe_code)]

//! Product reporting and custody around Psi-owned Terminal production.
//!
//! `TerminalProductionRequest::produce` adapts product timings and refusal
//! reporting to `psi_compiler`; it contains no lowering sequence.
//! Early compile-time jobs can depend on that Psi owner without importing this
//! Omega driver. Checked-source custody stays beside the portable artifact.

mod custody;
mod stage_timings;
mod terminal_production;
pub use custody::{
    ProducedTerminalArtifact, TerminalApplicationCustody, TerminalBehaviorExclusions,
    TerminalProductionCustody,
};
pub use psi_compiler::{PsiCompilationStage, TerminalMachineSelection, TerminalProgramRoots};
pub use stage_timings::PsiStageTimings;
pub use terminal_production::{
    ProgramEntryTerminalReceiptError, TerminalArtifactProductionError, TerminalProductionRequest,
};
