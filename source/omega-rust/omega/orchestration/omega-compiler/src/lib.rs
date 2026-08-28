//! Omega compilation coordination.
//!
//! The rooted API is [`Compiler`]. `public_api` temporarily preserves the
//! former flat domain exports while those models move to their owning crates.

mod compiler;
mod pipeline;
mod public_api;

pub use compiler::{
    ArtifactEmissionPolicy, CompileHarnessRequest, CompileOptions, CompileOutputKind,
    CompileReport, CompileRequest, Compiler, ExecutablePublicationDestination,
    ExecutablePublicationReceipt, RequestedCompileProduct, RetainedNativeArtifact,
    TerminalComponentDeploymentReportError, compile, compile_harness,
};
pub use public_api::*;

pub(crate) use psi_source as source;
pub(crate) use psi_source_files_to_tokens as lexer;
pub(crate) use psi_tokens_to_syntax_trees as parser;
