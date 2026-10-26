#![forbid(unsafe_code)]

//! The Psi driver's shared checked-program-to-Terminal operation.
//!
//! Early jobs run while source compilation is still settling types. Keeping
//! this operation below the source facade lets those jobs use exactly the same
//! lowering, optimization and publication as completed products, without a
//! dependency back into their own source coordinator. There is no second stage
//! sequence, source preparation or execution engine here.

mod build_bindings;
mod build_provider_selections;
pub use build_provider_selections::{TerminalBuildProviderCall, TerminalBuildProviderSelections};
mod build_filesystem;
mod build_protocol;
mod build_requests;
mod composition_admission;
mod custody;
mod observation;
mod terminal_production;

pub use build_bindings::{
    TerminalBuildBindingCall, TerminalBuildBindingOperation, TerminalBuildBindings,
};
pub use build_filesystem::{
    TerminalBuildFilesystem, TerminalBuildFilesystemCall, TerminalBuildFilesystemOperation,
};
pub use build_requests::{
    TerminalBuildRequestCall, TerminalBuildRequestOperation, TerminalBuildRequests,
};
pub use checked_trees_to_lowered_psi::{TerminalMachineSelection, TerminalProgramRoots};
pub use composition_admission::TerminalCompositionAdmission;
pub use custody::{ProducedTerminalProgram, TerminalApplicationCustody, TerminalProductionCustody};
pub use observation::{PsiCompilationObserver, PsiCompilationStage, UnmeasuredProduction};
pub use terminal_production::{
    ProgramEntryTerminalReceiptError, RetainedCallableSignature, TerminalArtifactProductionError,
    TerminalProductionRequest, retained_callable_signature,
};
