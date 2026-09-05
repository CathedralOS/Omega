#![forbid(unsafe_code)]

//! Psi-owned parsing of Omega tokens into unresolved source-shaped syntax.

pub mod parse_error;
pub mod parser;

pub use parse_error::ParseError;
pub use parser::{parse_syntax_trees, parse_syntax_trees_into_with_id, parse_syntax_trees_with_id};
