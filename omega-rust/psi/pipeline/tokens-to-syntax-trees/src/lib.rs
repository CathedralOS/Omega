#![forbid(unsafe_code)]

//! Psi-owned parsing of Omega tokens into unresolved source-shaped syntax.
//!
//! Start at `parser.rs`: `parse` owns source traversal, declaration dispatch,
//! and root publication into the caller's `SyntaxTrees`. Its sibling grammar
//! domains each own one file beside it: `declarations`, `expressions`,
//! `type_syntax`, `parameters`, `contracts` and `bodies` recognize their
//! part of the grammar over the `input` token cursor, and `diagnostics` owns
//! the failure surface. Meaning belongs to the resolution and typing stages
//! that follow; this stage only recognizes shape.

mod bodies;
mod contracts;
mod declarations;
mod diagnostics;
mod expressions;
mod input;
mod parameters;
pub mod parser;
mod type_syntax;

pub use diagnostics::parse_error::ParseError;
pub use parser::{
    parse, parse_syntax_trees, parse_syntax_trees_into_with_id, parse_syntax_trees_with_id,
};

#[cfg(test)]
mod tests;
