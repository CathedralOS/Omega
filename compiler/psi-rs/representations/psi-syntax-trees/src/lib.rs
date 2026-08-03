#![forbid(unsafe_code)]

//! Parsed Omega source shape before name and symbol resolution.

pub mod expression;
pub mod identifier;
pub mod identity;
pub mod item;
pub mod snapshot;
pub mod statement;
pub mod syntax_trees;
pub mod types;

pub use psi_language_core::operator_spelling;
pub use snapshot::SyntaxTreesSnapshot;
pub use syntax_trees::{SyntaxTreeRoots, SyntaxTreeTables, SyntaxTrees};
