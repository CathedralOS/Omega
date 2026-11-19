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
    let (mut pages, _) = parse_pages(source_id, tokens, 1)?.into_parts();
    Ok(pages.pop().expect("one source produces one syntax page"))
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
    let (pages, sources) = parse_pages(source_id, tokens, first_page)?.into_parts();
    let source = &sources.sources.span_or_empty(sources.batch)[0];
    let roots = ParsedRootItems {
        items: source.root_items.clone(),
        import_range: source.import_root_range.clone(),
    };
    for page in pages {
        syntax.adopt_page(page);
    }
    Ok(roots)
}

fn parse_pages(
    source_id: SourceId,
    tokens: &tokens::TokenStream<'_>,
    first_page_identity: u32,
) -> Result<syntax_trees::ParsedSyntaxPages, parsing::ParseError> {
    let source = tokens.shared_source();
    let mut lexed = tokens::LexedSources::default();
    lexed.batch = lexed.sources.insert_many([tokens::LexedSource {
        source_id,
        tokens: tokens.clone().retain_source(source),
        ..Default::default()
    }]);
    parsing::parse(
        &lexed,
        parsing::ParseOptions {
            first_page_identity,
            ..Default::default()
        },
    )
}
