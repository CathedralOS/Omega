/*
Parallel batch tokenization (`lex_sources`) over loaded in-memory source units
on the invocation's `execution_admission` budget,
preserving source order, origin metadata, and shared backing text while reporting
the first lexical failure with exact file, line, and column coordinates.
*/

use arena::Arena;
use diagnostics::Diagnostic;
use rayon::prelude::*;
use source::{LoadedSource, LoadedSources, SourcePosition};
use tokens::{LexedSource, LexedSources};

pub fn lex_sources(mut sources: LoadedSources) -> Result<LexedSources, Vec<Diagnostic>> {
    let loaded_sources = sources.sources.span_mut_or_empty(sources.batch);
    let source_count = loaded_sources.len();
    let mut lexed_sources = Arena::with_capacity(source_count);
    let results: Vec<Result<LexedSource, Diagnostic>> = if source_count <= 1 {
        loaded_sources.iter_mut().map(lex_source).collect()
    } else {
        execution_admission::within_budget(|| {
            loaded_sources.par_iter_mut().map(lex_source).collect()
        })
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
        tokens: tokens.retain_source(loaded_source.source.clone()),
        scope: loaded_source.scope,
    })
}
