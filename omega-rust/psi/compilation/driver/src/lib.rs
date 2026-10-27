#![forbid(unsafe_code)]

//! Psi compilation sequencing, independent of product scheduling and native output.
//!
//! Start at `source_parsing.rs` for admitted source frontiers: Psi owns loading,
//! lexing, parsing and page publication; packages own discovery and source custody.
//! `source_compilation.rs` consumes admitted syntax: `prepare` owns source
//! evaluation, resolution and the paired typing continuation. Build execution
//! borrows its declared inputs; `complete_authored` resumes that same owner,
//! and `extend` consumes admitted generated syntax against its exact predecessor.
//! Package acquisition, Build permissions and host effects stay outside this crate.
//!
//! `source_checking.rs` owns preparation, exact retained-judgment matching and
//! checking for both Product and Build. Successful checking precedes product
//! pruning; consumers supply admission facts, never a stage sequence.
//! `terminal-production/src/terminal_production.rs` owns checked trees through
//! stages 05-07 and canonical publication, including separate consumer admissions.
//! That shared operation sits below this source facade so early compile-time jobs
//! can use it without recursively importing their source coordinator. Source/package assembly,
//! source completion roles and distinct entry-root production remain outside
//! this facade; this is not yet a complete one-call source-to-Terminal API.
//! Observers record existing stage operations without choosing or replaying them.

mod source_checking;
mod source_compilation;
mod source_parsing;

pub use source_checking::{
    CheckRequest, CheckedSource, CheckedTreeProductSelection, ProductSelections, RetainedCheck,
    SelectedCheckedSource, SourceAdmission, settle_closed_constant_execution,
};
pub use source_parsing::{
    ParsedText, SourceInput, SourceParseError, load_source_files, parse_text,
};

pub use source_compilation::{
    BuildSourceInput, CompletedSource, PreparedSource, SourceCompilationRequest,
    SourceContinuation, SourceExtensionRequest, SourceFrontends,
};

pub use psi_terminal_production::{
    BuildProviderProductionRequest, ProducedBuildProvider, ProducedTerminalProgram,
    ProgramEntryTerminalReceiptError, PsiCompilationObserver, PsiCompilationStage,
    TerminalApplicationCustody, TerminalArtifactProductionError, TerminalBuildBindingCall,
    TerminalBuildBindingOperation, TerminalBuildBindings, TerminalBuildFilesystem,
    TerminalBuildFilesystemCall, TerminalBuildFilesystemOperation,
    TerminalBuildProviderRequirement, TerminalBuildQueryCall, TerminalBuildQueryOperation,
    TerminalBuildRequestCall, TerminalBuildRequestOperation, TerminalBuildRequests,
    TerminalBuildSelectionRequest, TerminalCompositionAdmission, TerminalMachineSelection,
    TerminalProductionCustody, TerminalProductionRequest, TerminalProgramRoots,
    UnmeasuredProduction,
};
