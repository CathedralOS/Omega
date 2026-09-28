pub use crate::diagnostics::parse_error;

use crate::ParseError;
use crate::declarations::parse_declaration::{ParsedDeclaration, parse_item};
use crate::input::token_cursor::Input;
use arena::Arena;
use diagnostics::Diagnostic;
use source::{SourceId, SourcePosition};
use syntax_trees::{ParsedSource, ParsedSources, SyntaxTrees, item::ItemHandle};
use tokens::{LexedSources, Token};

/// Parse an ordered token batch into the caller's syntax arena, retaining
/// per-source roots, backing text and metadata. Failure leaves the already
/// parsed prefix in that arena and reports the current source's exact location.
pub fn parse_sources(
    lexed: LexedSources,
    syntax_trees: &mut SyntaxTrees,
) -> Result<ParsedSources, Vec<Diagnostic>> {
    let lexed_sources = lexed.sources.span_or_empty(lexed.batch);
    let source_count = lexed_sources.len();
    let mut parsed_sources = Arena::with_capacity(source_count);
    let mut parsed = Vec::with_capacity(source_count);

    for lexed_source in lexed_sources {
        let root_items = parse(syntax_trees, lexed_source.source_id, &lexed_source.tokens)
            .map_err(|error| {
                let position =
                    SourcePosition::of(lexed_source.source.as_ref(), error.source_span.span.start);
                vec![Diagnostic::error(format!(
                    "{}:{}:{}: {}",
                    lexed_source.path.display(),
                    position.line,
                    position.column,
                    error.message
                ))]
            })?;

        parsed.push(ParsedSource {
            source_id: lexed_source.source_id,
            path: lexed_source.path.clone(),
            source: lexed_source.source.clone(),
            origin: lexed_source.origin,
            root_items,
            scope: lexed_source.scope,
        });
    }

    let batch = parsed_sources.insert_many(parsed);

    Ok(ParsedSources {
        sources: parsed_sources,
        batch,
    })
}

/// Parses one source into the caller's arena and returns that source's root
/// item handles. Top-level `let`/`boundary let` declarations land on
/// `SyntaxTreeRoots::mathematical_definitions` instead — they are not items —
/// so they do not appear in the returned list. Existing roots retain their
/// identities. On error, the arena retains the parsed prefix.
pub fn parse(
    syntax_trees: &mut SyntaxTrees,
    source_id: SourceId,
    tokens: &[Token<'_>],
) -> Result<Vec<ItemHandle>, ParseError> {
    let mut input = Input::new(source_id, tokens);
    let mut root_items = Vec::new();
    while !input.tokens.is_empty() {
        let (declaration, rest) = parse_item(syntax_trees, input)?;
        match declaration {
            ParsedDeclaration::Item(item) => {
                root_items.push(syntax_trees.push_root_item(item));
            }
            ParsedDeclaration::Mathematical(definition) => {
                syntax_trees.push_root_mathematical_definition(definition);
            }
        }
        input = rest;
    }

    Ok(root_items)
}

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
