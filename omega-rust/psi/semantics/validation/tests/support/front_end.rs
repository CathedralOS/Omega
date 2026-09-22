//! The front-end pipelines the validation tests run.
//!
//! A fixture names the pipeline it exercises by calling one of these functions
//! instead of re-sequencing lexing, parsing, resolution and typing by hand, so
//! a reader sees which stages ran before the validator under test and a
//! fixture cannot silently skip or add one. Every function's doc comment
//! states its exact stage sequence; a stage that fails panics with the stage
//! name and the source it ran on.
//!
//! `validation` reads typed trees, so every pipeline here stops at
//! `lower_symbol_resolved_trees`. Checking lives above this crate and is not
//! one of its dependencies, so there is no `checked_program` here.
//!
//! One file serves both test trees. The `suite` integration target includes it
//! as `mod front_end` (`tests/suite.rs`), and `src/lib.rs` includes the same
//! file under `#[cfg(test)]`, so unit tests and integration tests spell every
//! call `crate::front_end::..` and read one definition of the sequence.

// Both test trees include this one file, so each target sees the pipelines the
// other one calls.
#![allow(dead_code)]

use diagnostics::Diagnostic;
use source::{SourceId, SourceMap};
use source_files_to_tokens::Lexer;
use std::sync::Arc;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::pre_resolution::{
    GenericDataRequest, normalize_generic_data,
};
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::{
    parse_syntax_trees, parse_syntax_trees_into_with_id, parse_syntax_trees_with_id,
};
use typed_trees::TypedTrees;

/// Unwrap one stage's result, or panic with the stage name and the source it
/// ran on.
fn stage<T, E: std::fmt::Debug>(name: &str, source: &str, result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| panic!("{name}: {error:#?}\nsource:\n{source}"))
}

/// Lex, parse, resolve and type `source` as one anonymous source with no
/// source map.
pub(crate) fn typed_program(source: &str) -> TypedTrees {
    let tokens = stage("tokenize", source, Lexer::new(source).tokenize());
    let syntax = stage("parse", source, parse_syntax_trees(&tokens));
    let resolved = stage("resolve", source, resolve(ResolutionRequest::new(&syntax)));
    stage("type", source, lower_symbol_resolved_trees(&resolved))
}

/// Lex, parse, resolve and type `source` as one anonymous source with no
/// source map; lexing and parsing panic, a resolution or typing rejection is
/// returned, for the fixtures that assert on what the front end refuses.
pub(crate) fn typed_program_result(source: &str) -> Result<TypedTrees, Vec<Diagnostic>> {
    let tokens = stage("tokenize", source, Lexer::new(source).tokenize());
    let syntax = stage("parse", source, parse_syntax_trees(&tokens));
    let resolved = resolve(ResolutionRequest::new(&syntax))?;
    lower_symbol_resolved_trees(&resolved).map_err(|diagnostic| vec![diagnostic])
}

/// Lex and parse `source` as one anonymous source with no source map, close
/// its generic data applications with the pre-resolution rewrite
/// (`pre_resolution::normalize_generic_data`), then resolve and type. The
/// rewrite is the seam build-time evaluation drives, not an ordinary
/// front-end stage, so the fixtures that need it name it here.
pub(crate) fn typed_program_with_generic_data(source: &str) -> TypedTrees {
    let tokens = stage("tokenize", source, Lexer::new(source).tokenize());
    let syntax = stage("parse", source, parse_syntax_trees(&tokens));
    let syntax = stage(
        "normalize generic data",
        source,
        normalize_generic_data(GenericDataRequest::new(syntax)),
    );
    let resolved = stage("resolve", source, resolve(ResolutionRequest::new(&syntax)));
    stage("type", source, lower_symbol_resolved_trees(&resolved))
}

/// Lex and parse every text in order into one syntax forest, the text at index
/// `ordinal` under `SourceId(ordinal)`, then resolve with no source map and
/// type. The fixture owns the texts and their order; the sequence is what is
/// shared. The first text names the source in resolution and typing failures.
pub(crate) fn typed_program_from_texts(texts: &[&str]) -> TypedTrees {
    let (first, rest) = texts
        .split_first()
        .expect("a multi-source fixture parses at least one text");
    let tokens = stage("tokenize", first, Lexer::new(first).tokenize());
    let mut syntax = stage("parse", first, parse_syntax_trees(&tokens));
    for (source_ordinal, text) in rest.iter().enumerate() {
        let tokens = stage("tokenize", text, Lexer::new(text).tokenize());
        stage(
            "parse",
            text,
            parse_syntax_trees_into_with_id(&mut syntax, SourceId(source_ordinal + 1), &tokens),
        );
    }
    let resolved = stage("resolve", first, resolve(ResolutionRequest::new(&syntax)));
    stage("type", first, lower_symbol_resolved_trees(&resolved))
}

/// Lex and parse each `(source_id, text)` in order into one syntax forest,
/// resolve it against `sources`, then type. The fixture registers its sources
/// (paths, roots, packages, origins) in the map itself; the sequence is what
/// is shared. The last text is the fixture's own program and names the source
/// in resolution and typing failures.
pub(crate) fn typed_program_from_source_map(
    sources: SourceMap,
    texts: &[(SourceId, &str)],
) -> TypedTrees {
    let program = last_text(texts);
    stage(
        "type",
        program,
        typed_program_from_source_map_typing_result(sources, texts),
    )
}

/// [`typed_program_from_source_map`], except that only the typing rejection is
/// returned: lexing, parsing and resolution still panic. The fixtures that use
/// this assert on a typing diagnostic and treat an earlier failure as a broken
/// fixture.
pub(crate) fn typed_program_from_source_map_typing_result(
    sources: SourceMap,
    texts: &[(SourceId, &str)],
) -> Result<TypedTrees, Diagnostic> {
    let ((first_id, first), rest) = texts
        .split_first()
        .expect("a source-map fixture parses at least one source");
    let tokens = stage("tokenize", first, Lexer::new(first).tokenize());
    let mut syntax = stage(
        "parse",
        first,
        parse_syntax_trees_with_id(*first_id, &tokens),
    );
    for (source_id, text) in rest {
        let tokens = stage("tokenize", text, Lexer::new(text).tokenize());
        stage(
            "parse",
            text,
            parse_syntax_trees_into_with_id(&mut syntax, *source_id, &tokens),
        );
    }
    let resolved = stage(
        "resolve",
        last_text(texts),
        resolve(ResolutionRequest {
            syntax: &syntax,
            sources: Some(Arc::new(sources)),
            top_level_bindings: Vec::new(),
        }),
    );
    lower_symbol_resolved_trees(&resolved)
}

/// Lex and parse each `(source_id, text)` into its own syntax forest, merge
/// them with `SyntaxTrees::extend_from`, resolve the merged forest against
/// `sources`, then type. Merging is not the same as parsing into one arena:
/// `extend_from` lands every copied mathematical declaration after the copied
/// items, so the fixtures that read that order keep this pipeline.
pub(crate) fn typed_program_from_merged_source_map(
    sources: SourceMap,
    texts: &[(SourceId, &str)],
) -> TypedTrees {
    let ((first_id, first), rest) = texts
        .split_first()
        .expect("a source-map fixture parses at least one source");
    let tokens = stage("tokenize", first, Lexer::new(first).tokenize());
    let mut syntax = stage(
        "parse",
        first,
        parse_syntax_trees_with_id(*first_id, &tokens),
    );
    for (source_id, text) in rest {
        let tokens = stage("tokenize", text, Lexer::new(text).tokenize());
        let forest = stage(
            "parse",
            text,
            parse_syntax_trees_with_id(*source_id, &tokens),
        );
        syntax.extend_from(&forest);
    }
    let program = last_text(texts);
    let resolved = stage(
        "resolve",
        program,
        resolve(ResolutionRequest {
            syntax: &syntax,
            sources: Some(Arc::new(sources)),
            top_level_bindings: Vec::new(),
        }),
    );
    stage("type", program, lower_symbol_resolved_trees(&resolved))
}

/// The fixture's own program: the last text a source-map pipeline parsed.
fn last_text<'a>(texts: &[(SourceId, &'a str)]) -> &'a str {
    texts
        .last()
        .expect("a source-map fixture parses at least one source")
        .1
}
