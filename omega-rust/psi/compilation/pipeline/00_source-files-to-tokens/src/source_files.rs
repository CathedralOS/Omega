//! Load a source frontier, then tokenize its files in their supplied order.
//!
//! Loading and lexing remain separate operations: all file reads finish before
//! the first lexical error can be reported. In-memory inputs use the same
//! lexing operation without acquiring filesystem or package authority.

use arena::Arena;
use diagnostics::Diagnostic;
use source::{
    DependencyScope, LoadedSource, LoadedSources, SourceId, SourceOrigin, SourcePosition,
};
use std::path::PathBuf;
use std::sync::Arc;
use tokens::{LexedSource, LexedSources, Token, TokenStream, TokenText};

/// Read one ordered frontier, assigning consecutive source coordinates.
/// Equal paths in different dependency scopes remain separate source instances.
pub fn load_sources(
    frontier: Vec<(PathBuf, DependencyScope)>,
    first_source_id: usize,
) -> Result<LoadedSources, Vec<Diagnostic>> {
    let source_count = frontier.len();
    let mut sources = Arena::with_capacity(source_count);
    let mut loaded = Vec::with_capacity(source_count);

    for (source_index, (path, scope)) in frontier.into_iter().enumerate() {
        let source = std::fs::read_to_string(&path).map_err(|error| {
            vec![Diagnostic::error(format!(
                "failed to read {}: {error}",
                path.display()
            ))]
        })?;
        source::read_accounting::record_source_file_read(
            "frontend_load",
            &path,
            source.len() as u64,
        );

        loaded.push(LoadedSource {
            source_id: SourceId(first_source_id + source_index),
            path,
            source: Arc::from(source),
            origin: None,
            scope,
        });
    }

    let batch = sources.insert_many(loaded);

    Ok(LoadedSources { sources, batch })
}

/// Load a COMPILER-PROVIDED source (a virtual file with no on-disk backing):
/// the build-vocabulary prelude and its future siblings. The synthetic path
/// names the provider in diagnostics.
pub fn load_injected_source(name: &str, text: &str, first_source_id: usize) -> LoadedSources {
    let mut sources = Arena::with_capacity(1);
    // Injected toolchain vocabulary is compiler-owned, not a dependency-
    // selected instance; it carries product scope and joins a build entry
    // through its explicit source-scoped binding instead.
    let batch = sources.insert_many([LoadedSource {
        source_id: SourceId(first_source_id),
        path: PathBuf::from(name),
        source: Arc::from(text),
        origin: Some(SourceOrigin::Toolchain),
        scope: DependencyScope::Product,
    }]);
    LoadedSources { sources, batch }
}

/// Load one compiler-retained package source with a logical package-relative
/// path and no physical file access. `scope` is the checked instance the
/// generated bundle joins: a bundle embodies its producing package's own
/// context (host context for a build-only package), so generated sources
/// keep their owning package's scope rather than the importer's.
pub fn load_package_generated_source(
    path: PathBuf,
    text: &str,
    source_id: usize,
    scope: DependencyScope,
) -> LoadedSources {
    let mut sources = Arena::with_capacity(1);
    let batch = sources.insert_many([LoadedSource {
        source_id: SourceId(source_id),
        path,
        source: Arc::from(text),
        origin: Some(SourceOrigin::User),
        scope,
    }]);
    LoadedSources { sources, batch }
}

/// Tokenize every loaded source, retaining its backing text and exact metadata.
/// Report the first lexical failure with its source path, line and column.
pub fn lex_sources(sources: LoadedSources) -> Result<LexedSources, Vec<Diagnostic>> {
    let loaded_sources = sources.sources.span_or_empty(sources.batch);
    let source_count = loaded_sources.len();
    let mut lexed_sources = Arena::with_capacity(source_count);
    let mut lexed = Vec::with_capacity(source_count);

    for loaded_source in loaded_sources {
        let tokens = crate::Lexer::new(loaded_source.source.as_ref())
            .tokenize()
            .map_err(|error| {
                let position = SourcePosition::of(loaded_source.source.as_ref(), error.span.start);
                vec![Diagnostic::error(format!(
                    "{}:{}:{}: {}",
                    loaded_source.path.display(),
                    position.line,
                    position.column,
                    error.message
                ))]
            })?;

        lexed.push(LexedSource {
            source_id: loaded_source.source_id,
            path: loaded_source.path.clone(),
            source: loaded_source.source.clone(),
            origin: loaded_source.origin,
            tokens: own_token_stream(tokens, &loaded_source.source),
            scope: loaded_source.scope,
        });
    }

    let batch = lexed_sources.insert_many(lexed);

    Ok(LexedSources {
        sources: lexed_sources,
        batch,
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
