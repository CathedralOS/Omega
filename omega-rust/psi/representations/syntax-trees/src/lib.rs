#![forbid(unsafe_code)]

//! Parsed Omega source shape before name and symbol resolution.
//!
//! Start at `syntax_trees::SyntaxTrees`; its modules own the program's concepts:
//! items, statements, expressions, types, names, and inspection snapshots.
//! Identifiers are spellings with spans, not symbols, and no expression carries
//! a type; both are assigned by the stages that consume this shape.

pub mod syntax_trees;

pub use syntax_trees::control_flow::statement;
pub use syntax_trees::declarations::item;
pub use syntax_trees::inspection::snapshot;
pub use syntax_trees::names::{identifier, identity};
pub use syntax_trees::type_system::types;
pub use syntax_trees::values::expression;

pub use language_core::operator_spelling;
pub use snapshot::SyntaxTreesSnapshot;
pub use syntax_trees::{SyntaxTreeRoots, SyntaxTreeTables, SyntaxTrees};
