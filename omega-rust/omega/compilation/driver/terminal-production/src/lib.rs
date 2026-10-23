#![forbid(unsafe_code)]

//! The product adapters around Psi-owned Terminal production.
//!
//! `produce_terminal_artifact` supplies this product's timing recorder and its
//! production counter to `psi_compiler`'s one `TerminalProductionRequest`
//! operation; the request and the application custody rows are Psi's, re-exported
//! here so a product caller names one entry. The product's exclusion policy
//! implements Psi's composition admission. Early compile-time jobs can depend on
//! that Psi owner without importing this Omega driver. Checked-source custody
//! stays beside the portable artifact.

mod policy;
mod stage_timings;
mod terminal_production;
pub use policy::TerminalBehaviorExclusions;
pub use psi_compiler::{
    ProducedTerminalArtifact, TerminalApplicationCustody, TerminalMachineSelection,
    TerminalProductionRequest, TerminalProgramRoots,
};
pub use stage_timings::PsiStageTimings;
pub use terminal_production::{
    ProgramEntryTerminalReceiptError, TerminalArtifactProductionError, produce_terminal_artifact,
};
