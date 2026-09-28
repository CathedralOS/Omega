//! Source-set composition: from source files to one assembled syntax forest.
//!
//! Start at `source_assembly.rs`. This preparation discovers the project's roots
//! and imports, loads and lexes and parses every source with its package
//! custody, injects the build prelude, and returns the forest the next stage
//! resolves. The build continuation directly feeds generated units through
//! the same Psi stages after package source admission. Package
//! compilation owns source roots, import selection and parsed-file custody;
//! Psi stages 00 and 01 own loading, lexing and parsing. This coordinator keeps
//! that sequence and the build-specific prelude/entry injection visible.

mod source_assembly;

pub use source_assembly::{AssembledSyntax, ImmutableSourceParseCheckpoint};
