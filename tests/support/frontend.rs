//! Fixture construction through the public lexical and parsing operations.
#![allow(dead_code)]
use source::{LoadedSource, SourceId};
use std::sync::Arc;
use syntax_trees::{ParsedRootItems, SyntaxTrees};

pub struct Lexer<'text>(&'text str);
impl<'text> Lexer<'text> {
    pub fn new(text: &'text str) -> Self {
        Self(text)
    }
    pub fn tokenize(
        self,
    ) -> Result<tokens::TokenStream<'static>, source_files_to_tokens::LexError> {
        lex_retained(Arc::from(self.0))
    }
}

pub fn lex_retained(
    source: Arc<str>,
) -> Result<tokens::TokenStream<'static>, source_files_to_tokens::LexError> {
    let loaded = [LoadedSource {
        source,
        ..Default::default()
    }]
    .into_iter()
    .collect();
    let mut lexed = source_files_to_tokens::lex(loaded)?;
    Ok(std::mem::take(
        &mut lexed.sources.span_mut_or_empty(lexed.batch)[0].tokens,
    ))
}

pub fn parse_syntax_trees(
    tokens: &tokens::TokenStream<'_>,
) -> Result<SyntaxTrees, tokens_to_syntax_trees::ParseError> {
    parse_syntax_trees_with_id(SourceId::default(), tokens)
}

pub fn parse_syntax_trees_with_id(
    source_id: SourceId,
    tokens: &tokens::TokenStream<'_>,
) -> Result<SyntaxTrees, tokens_to_syntax_trees::ParseError> {
    let (mut pages, _) = parse_pages(source_id, tokens, 1)?.into_parts();
    Ok(pages.pop().expect("one source produces one syntax page"))
}

pub fn parse(
    syntax: &mut SyntaxTrees,
    source_id: SourceId,
    tokens: &tokens::TokenStream<'_>,
) -> Result<ParsedRootItems, tokens_to_syntax_trees::ParseError> {
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
) -> Result<syntax_trees::ParsedSyntaxPages, tokens_to_syntax_trees::ParseError> {
    let source = tokens
        .source_owner()
        .cloned()
        .unwrap_or_else(|| Arc::from(tokens.source_text()));
    let mut lexed = tokens::LexedSources::default();
    lexed.batch = lexed.sources.insert_many([tokens::LexedSource {
        source_id,
        source: source.clone(),
        tokens: tokens.clone().retain_source(source),
        ..Default::default()
    }]);
    tokens_to_syntax_trees::parse(
        &lexed,
        tokens_to_syntax_trees::ParseOptions {
            first_page_identity,
            ..Default::default()
        },
    )
}
