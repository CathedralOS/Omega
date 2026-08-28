//! Omega compilation coordination.
//!
//! The rooted API is [`Compiler`]. Domain models are imported from their
//! owning stage and orchestration crates rather than republished here.

mod compiler;
mod pipeline;

pub use compiler::{
    ArtifactEmissionPolicy, CompileOptions, CompileOutputKind, CompileReport, CompileRequest,
    Compiler, ExecutablePublicationDestination, ExecutablePublicationReceipt,
    RequestedCompileProduct, RetainedNativeArtifact, TerminalComponentDeploymentReportError,
    compile,
};
pub use pipeline::checked_entry::{
    CheckedCompilation, compile_to_checked, compile_to_checked_with_packages,
    compile_to_checked_with_packages_and_replay_record,
    compile_to_checked_with_packages_in_build_dir,
    compile_to_checked_with_packages_in_sponsored_build_dir, compile_to_checked_with_replay_record,
};
pub use pipeline::source_inspection::{
    PackageSourceClosureCustodySnapshot, SOURCE_CLOSURE_SNAPSHOT_SCHEMA, SourceClosureSnapshot,
    SourceClosureSnapshotEntry, SourceClosureSnapshotFingerprint, SourceInspectionRoot,
    inspect_source_closure, inspect_source_closure_with_packages,
};
pub(crate) use psi_source as source;
pub(crate) use psi_source_files_to_tokens as lexer;
pub(crate) use psi_tokens_to_syntax_trees as parser;
