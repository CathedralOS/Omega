#![forbid(unsafe_code)]

//! Psi compilation sequencing, independent of product scheduling and native output.
//!
//! Start at `source_compilation.rs` for admitted syntax: `prepare` owns source
//! evaluation, resolution and the paired typing continuation. Build execution
//! borrows its declared inputs; `complete_authored` resumes that same owner,
//! and `extend` consumes admitted generated syntax against its exact predecessor.
//! Package acquisition, Build permissions and host effects stay outside this crate.
//!
//! `source_checking.rs` owns preparation, exact retained-judgment matching and
//! checking for both Product and Build. Successful checking precedes product
//! pruning; consumers supply admission facts, never a stage sequence.
//! `terminal_production.rs` owns checked trees through stages 05-07 and canonical
//! publication, including separate consumer admissions. Source/package assembly,
//! source completion roles and distinct entry-root production remain outside
//! this facade; this is not yet a complete one-call source-to-Terminal API.
//! Observers record existing stage operations without choosing or replaying them.

mod composition_admission;
mod custody;
pub use composition_admission::TerminalCompositionAdmission;
mod observation;
mod source_checking;
mod source_compilation;
mod terminal_production;

pub use source_checking::{
    CheckRequest, CheckedSource, CheckedTreeProductSelection, RetainedCheck, SelectedCheckedSource,
    SourceAdmission,
};

pub use source_compilation::{
    BuildSourceInput, CompletedSource, PreparedSource, SourceCompilationRequest,
    SourceContinuation, SourceExtensionRequest,
};

pub use checked_trees_to_lowered_psi::TerminalMachineSelection;
pub use custody::{
    ProducedTerminalArtifact, TerminalApplicationCustody, TerminalProductionCustody,
};
pub use observation::{PsiCompilationObserver, PsiCompilationStage, UnmeasuredProduction};
pub use terminal_production::{
    ProgramEntryTerminalReceiptError, TerminalArtifactProductionError, TerminalProductionRequest,
};
