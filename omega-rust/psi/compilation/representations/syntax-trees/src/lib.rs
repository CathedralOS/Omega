#![forbid(unsafe_code)]

//! Parsed Omega source shape before name and symbol resolution.
//!
//! Start at `syntax_trees::SyntaxTrees`; its modules own the program's concepts:
//! items, statements, expressions, types, names, and inspection snapshots.
//! Identifiers are spellings with spans, not symbols, and no expression carries
//! a type; both are assigned by the stages that consume this shape.
//!
//! Each forest retains source pages once. Authored name and float-spelling occurrences use page
//! handles and spans; literal bytes share the lexer's decoded buffer through
//! later representations. Cross-forest copies retain the referenced pages before
//! copying nodes. Complete forest equality resolves names through both owners;
//! its derived structural walk includes detached arena rows and allocation
//! metadata, preserving Build continuation-sharing decisions.

mod equality;
pub use equality::SyntaxEquivalent;

pub mod syntax_trees;

pub use syntax_trees::control_flow::statement;
pub use syntax_trees::declarations::item;
pub use syntax_trees::inspection::snapshot;
pub use syntax_trees::names::identifier;
pub use syntax_trees::type_system::types;
pub use syntax_trees::values::expression;
pub use syntax_trees::values::float_spelling::FloatSpelling;

pub use language_core::operator_spelling;
pub use snapshot::SyntaxTreesSnapshot;
pub use syntax_trees::source_files::{ParsedRootItems, ParsedSource, ParsedSources};
pub use syntax_trees::{SyntaxTreeRoots, SyntaxTreeTables, SyntaxTrees};

pub use syntax_trees::names::text_owner::{RetainedSource, SyntaxTextContext, SyntaxTextOwner};
