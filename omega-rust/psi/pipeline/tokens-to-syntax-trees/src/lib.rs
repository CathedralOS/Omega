#![forbid(unsafe_code)]

//! Psi-owned parsing of Omega tokens into unresolved source-shaped syntax.
//!
//! Start at `parser.rs`: `parse` owns source traversal, declaration dispatch,
//! and root publication into the caller's `SyntaxTrees`. Its sibling grammar
//! domains own declarations, expressions, type syntax, parameters, contracts,
//! and bodies. Diagnostics own the failure surface. Meaning belongs to
//! the resolution and typing stages that follow; this stage only recognizes shape.

mod bodies;
mod contracts;
mod declarations;
mod diagnostics;
mod expressions;
mod input;
mod parameters;
mod type_syntax;

pub mod parser;

pub use diagnostics::parse_error::ParseError;
pub use parser::parse;
pub use parser::parse as parse_syntax_trees_into_with_id;

use source::SourceId;
use syntax_trees::SyntaxTrees;
use tokens::Token;

/// Creates a syntax arena for an anonymous source and parses its tokens.
pub fn parse_syntax_trees(tokens: &[Token<'_>]) -> Result<SyntaxTrees, ParseError> {
    parse_syntax_trees_with_id(SourceId::default(), tokens)
}

/// Creates a syntax arena for one identified source and parses its tokens.
pub fn parse_syntax_trees_with_id(
    source_id: SourceId,
    tokens: &[Token<'_>],
) -> Result<SyntaxTrees, ParseError> {
    let mut syntax_trees = SyntaxTrees::new(source_id);
    parse(&mut syntax_trees, source_id, tokens)?;
    Ok(syntax_trees)
}
