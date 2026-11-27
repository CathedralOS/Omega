//! Fixture construction through the public lexical and parsing operations.
#![allow(dead_code)]
use super::{LoadedSource, lexing, parsing};
use source_text::SourceId;
use syntax_trees::{ParsedRootItems, SyntaxTrees};

#[path = "lexing.rs"]
mod lexical;
pub use lexical::*;

pub fn parse_syntax_trees(
    tokens: &tokens::TokenStream<'_>,
) -> Result<SyntaxTrees, parsing::ParseError> {
    parse_syntax_trees_with_id(SourceId::default(), tokens)
}

pub fn parse_syntax_trees_with_id(
    source_id: SourceId,
    tokens: &tokens::TokenStream<'_>,
) -> Result<SyntaxTrees, parsing::ParseError> {
    let mut syntax = SyntaxTrees::new(source_id);
    parsing::parse_into(&mut syntax, source_id, tokens)?;
    Ok(syntax)
}

pub fn parse(
    syntax: &mut SyntaxTrees,
    source_id: SourceId,
    tokens: &tokens::TokenStream<'_>,
) -> Result<ParsedRootItems, parsing::ParseError> {
    let first_page = u32::try_from(syntax.page_count())
        .unwrap()
        .checked_add(1)
        .unwrap();
    let mut page = SyntaxTrees::new_with_generation(source_id, first_page);
    let roots = parsing::parse_into(&mut page, source_id, tokens)?;
    syntax.adopt_page(page);
    Ok(roots)
}
