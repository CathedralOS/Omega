//! Load a source frontier, then tokenize its files in their supplied order.
//!
//! Loading and lexing remain separate operations: all file reads finish before
//! the first lexical error can be reported. In-memory inputs use the same
//! lexing operation without acquiring filesystem or package authority.

use arena::Arena;
use diagnostics::Diagnostic;
use rayon::prelude::*;
use source::{
    DependencyScope, LoadedSource, LoadedSources, SourceFileSystem, SourceId, SourcePosition,
};
use std::path::PathBuf;
use std::sync::Arc;
use tokens::{LexedSource, LexedSources, Token, TokenStream, TokenText};

/// Read one ordered frontier, assigning consecutive source coordinates.
/// Equal paths in different dependency scopes remain separate source instances.
/// The supplied view owns backing selection; captured reads never reopen files.
pub fn load_sources(
    frontier: Vec<(PathBuf, DependencyScope)>,
    first_source_id: usize,
    view: &SourceFileSystem,
) -> Result<LoadedSources, Vec<Diagnostic>> {
    let source_count = frontier.len();
    let mut sources = Arena::with_capacity(source_count);
    let mut loaded = Vec::with_capacity(source_count);

    for (source_index, (path, scope)) in frontier.into_iter().enumerate() {
        let source = view.read_to_string(&path).map_err(|error| {
            vec![Diagnostic::error(format!(
                "failed to read {}: {error}",
                path.display()
            ))]
        })?;

        loaded.push(LoadedSource {
            source_id: SourceId(first_source_id + source_index),
            path,
            source,
            origin: None,
            scope,
        });
    }

    let batch = sources.insert_many(loaded);

    Ok(LoadedSources { sources, batch })
}

/// Tokenize every loaded source, retaining its backing text and exact metadata.
/// Report the first lexical failure with its source path, line and column.
pub fn lex_sources(sources: LoadedSources) -> Result<LexedSources, Vec<Diagnostic>> {
    let loaded_sources = sources.sources.span_or_empty(sources.batch);
    let source_count = loaded_sources.len();
    let mut lexed_sources = Arena::with_capacity(source_count);
    let results: Vec<Result<LexedSource, Diagnostic>> = if source_count <= 1 {
        loaded_sources.iter().map(lex_source).collect()
    } else {
        loaded_sources.par_iter().map(lex_source).collect()
    };
    let lexed = results
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| vec![error])?;

    let batch = lexed_sources.insert_many(lexed);

    Ok(LexedSources {
        sources: lexed_sources,
        batch,
    })
}

fn lex_source(loaded_source: &LoadedSource) -> Result<LexedSource, Diagnostic> {
    let tokens = crate::Lexer::new(loaded_source.source.as_ref())
        .tokenize()
        .map_err(|error| {
            let position = SourcePosition::of(loaded_source.source.as_ref(), error.span.start);
            Diagnostic::error(format!(
                "{}:{}:{}: {}",
                loaded_source.path.display(),
                position.line,
                position.column,
                error.message
            ))
        })?;

    Ok(LexedSource {
        source_id: loaded_source.source_id,
        path: loaded_source.path.clone(),
        source: loaded_source.source.clone(),
        origin: loaded_source.origin,
        tokens: own_token_stream(tokens, &loaded_source.source),
        scope: loaded_source.scope,
    })
}

/// Retain this source's lexer output beyond its borrow, moving decoded literal
/// buffers. Individual shared lexemes also supply source ownership to the
/// parser's independently retained identifiers and source text.
fn own_token_stream(tokens: TokenStream<'_>, source: &Arc<str>) -> TokenStream<'static> {
    TokenStream::new(
        tokens
            .into_tokens()
            .into_iter()
            .map(|token| {
                let lexeme = match token.lexeme {
                    TokenText::Source(_) => TokenText::shared(source.clone(), token.span),
                    TokenText::Shared { source, span } => TokenText::Shared { source, span },
                    TokenText::Owned(value) => TokenText::Owned(value),
                    TokenText::OwnedBytes(value) => TokenText::OwnedBytes(value),
                };
                Token {
                    kind: token.kind,
                    lexeme,
                    span: token.span,
                }
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests;
