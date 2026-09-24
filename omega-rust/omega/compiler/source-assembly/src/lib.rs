#![forbid(unsafe_code)]

//! Compiler orchestration from source files to one assembled syntax forest.
//!
//! Start at `source_assembly.rs`. This compiler preparation discovers the project's roots
//! and imports, loads and lexes and parses every source with its package
//! custody, injects the build prelude, and returns the forest the next stage
//! resolves. It re-enters once per build that generates source, appending
//! the generated units to a retained checkpoint of the parsed base. The
//! `frontend` folder owns loading, lexing, parsing and import binding; the
//! `source` folder owns source storage, import queues and project roots.

pub(crate) mod frontend;
pub mod source;
mod source_assembly;

pub use source_assembly::{
    AssembledSyntax, ExactTargetSourceAssembly, ImmutableSourceParseCheckpoint,
    RetainedGeneratedSyntaxExtension, retain_generated_syntax_extension,
};
