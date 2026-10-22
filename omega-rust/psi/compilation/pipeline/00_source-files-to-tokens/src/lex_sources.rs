//! Tokenize every loaded source text, in parallel, keeping their order.
//!
//! The texts arrive already in memory, read from files by `load_sources` or
//! supplied directly, so lexing never touches the filesystem or acquires
//! package authority. Consumed rows transfer their path buffers to the next
//! stage; only the source text needs shared ownership for retained lexemes.

use arena::Arena;
use diagnostics::Diagnostic;
use rayon::prelude::*;
use source::{LoadedSource, LoadedSources, SourcePosition};
use std::sync::Arc;
use tokens::{LexedSource, LexedSources, Token, TokenStream, TokenText};

/// Tokenize every loaded source, retaining its backing text and exact metadata.
/// Report the first lexical failure with its source path, line and column.
pub fn lex_sources(mut sources: LoadedSources) -> Result<LexedSources, Vec<Diagnostic>> {
    let loaded_sources = sources.sources.span_mut_or_empty(sources.batch);
    let source_count = loaded_sources.len();
    let mut lexed_sources = Arena::with_capacity(source_count);
    let results: Vec<Result<LexedSource, Diagnostic>> = if source_count <= 1 {
        loaded_sources.iter_mut().map(lex_source).collect()
    } else {
        loaded_sources.par_iter_mut().map(lex_source).collect()
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

fn lex_source(loaded_source: &mut LoadedSource) -> Result<LexedSource, Diagnostic> {
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
        path: std::mem::take(&mut loaded_source.path),
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
