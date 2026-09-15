#![forbid(unsafe_code)]

//! Psi-owned parsing of Omega tokens into unresolved source-shaped syntax.
//!
//! Start at `parser.rs`: `parse` owns source traversal, declaration dispatch,
//! and root publication into the caller's `SyntaxTrees`; the closed
//! failure surface sits beneath it as `parser::parse_error`. Meaning belongs to
//! the resolution and typing stages that follow; this stage only recognizes shape.

pub mod parser;

pub use parser::parse;
pub use parser::parse as parse_syntax_trees_into_with_id;
pub use parser::parse_error::ParseError;

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
