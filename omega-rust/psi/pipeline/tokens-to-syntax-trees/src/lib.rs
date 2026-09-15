#![forbid(unsafe_code)]

//! Psi-owned parsing of Omega tokens into unresolved source-shaped syntax.
//!
//! Start at `parser.rs`: `parse_syntax_trees` drives one construct module per
//! language form and returns `SyntaxTrees` with no name resolved; the closed
//! failure surface sits beneath it as `parser::parse_error`. Meaning belongs to
//! the resolution and typing stages that follow; this stage only recognizes shape.

pub mod parser;

pub use parser::parse_error::ParseError;
pub use parser::{parse_syntax_trees, parse_syntax_trees_into_with_id, parse_syntax_trees_with_id};
