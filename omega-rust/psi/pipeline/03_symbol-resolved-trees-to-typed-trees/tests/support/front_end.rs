//! The front-end pipelines the typing tests run.
//!
//! A fixture names the pipeline it exercises by calling one of these functions
//! instead of re-sequencing lexing, parsing and resolution by hand, so a reader
//! sees which stages ran before `lower_symbol_resolved_trees` and a fixture
//! cannot silently skip or add one. Every function's doc comment states its
//! exact stage sequence; a stage that fails panics with the stage name and the
//! source it ran on.
//!
//! This crate is the typing stage, so the pipelines here stop at resolution or
//! at this crate's own `lower_symbol_resolved_trees`. Checking lives above this
//! crate and is not one of its dependencies, so there is no `checked_program`
//! here. A fixture that observes or rewrites the resolved trees before typing
//! takes [`resolved_program`] and calls `lower_symbol_resolved_trees` itself:
//! the stage under test stays visible at the test.
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
use symbol_resolved_trees::SymbolResolvedTrees;
// `symbol_resolved_trees_to_typed_trees` names this crate from the `suite`
// integration target and, through `extern crate self as ..` in `src/lib.rs`,
// from the crate's own unit tests, so one spelling serves both trees.
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees::SyntaxTrees;
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

/// Lex, parse and resolve `source` as one anonymous source with no source map,
/// for the fixtures that read or rewrite the resolved trees before typing them.
pub(crate) fn resolved_program(source: &str) -> SymbolResolvedTrees {
    let tokens = stage("tokenize", source, Lexer::new(source).tokenize());
    let syntax = stage("parse", source, parse_syntax_trees(&tokens));
    stage("resolve", source, resolve(ResolutionRequest::new(&syntax)))
}

/// Lex and parse `source` as one anonymous source with no source map, close its
/// generic data applications with the pre-resolution rewrite
/// (`pre_resolution::normalize_generic_data`), then resolve. The rewrite runs
/// ahead of resolution rather than inside it, so the fixtures that need the
/// synthesized declarations name it here.
pub(crate) fn resolved_program_with_generic_data(source: &str) -> SymbolResolvedTrees {
    let tokens = stage("tokenize", source, Lexer::new(source).tokenize());
    let syntax = stage("parse", source, parse_syntax_trees(&tokens));
    let syntax = stage(
        "normalize generic data",
        source,
        normalize_generic_data(GenericDataRequest::new(syntax)),
    );
    stage("resolve", source, resolve(ResolutionRequest::new(&syntax)))
}

/// Lex, parse, resolve and type `source` as one anonymous source with no
/// source map.
pub(crate) fn typed_program(source: &str) -> TypedTrees {
    typed_program_with_resolution(source).1
}

/// [`typed_program`], also handing back the resolved trees typing consumed, for
/// the fixtures that assert on resolution and on typing in one test.
pub(crate) fn typed_program_with_resolution(source: &str) -> (SymbolResolvedTrees, TypedTrees) {
    let resolved = resolved_program(source);
    let typed = stage("type", source, lower_symbol_resolved_trees(&resolved));
    (resolved, typed)
}

/// Lex, parse, resolve and type `source` as one anonymous source with no
/// source map; lexing, parsing and resolution panic, the typing rejection is
/// returned, for the fixtures that assert on what typing refuses.
pub(crate) fn typed_program_result(source: &str) -> Result<TypedTrees, Diagnostic> {
    lower_symbol_resolved_trees(&resolved_program(source))
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

/// Lex and parse each `(source_id, text)` in order into one syntax forest, then
/// resolve it against `sources`. The fixture registers its sources (paths,
/// roots, packages, origins, strata) in the map itself; the sequence is what is
/// shared. The last text is the fixture's own program and names the source in
/// resolution failures.
pub(crate) fn resolved_program_from_source_map(
    sources: SourceMap,
    texts: &[(SourceId, &str)],
) -> SymbolResolvedTrees {
    let (syntax, program) = parsed_forest(texts);
    resolved_forest(sources, &syntax, program)
}

/// [`resolved_program_from_source_map`] with generic data closed
/// (`pre_resolution::normalize_generic_data`) between parsing and resolution.
pub(crate) fn resolved_program_from_source_map_with_generic_data(
    sources: SourceMap,
    texts: &[(SourceId, &str)],
) -> SymbolResolvedTrees {
    let (syntax, program) = parsed_forest(texts);
    let syntax = stage(
        "normalize generic data",
        program,
        normalize_generic_data(GenericDataRequest::new(syntax)),
    );
    resolved_forest(sources, &syntax, program)
}

/// Lex and parse each `(source_id, text)` into its own syntax forest, merge
/// them with `SyntaxTrees::extend_from`, then resolve the merged forest against
/// `sources`. Merging is not the same as parsing into one arena: `extend_from`
/// lands every copied item after the forest it extends, so the fixtures that
/// read that order keep this pipeline.
pub(crate) fn resolved_program_from_merged_source_map(
    sources: SourceMap,
    texts: &[(SourceId, &str)],
) -> SymbolResolvedTrees {
    let ((first_id, first), rest) = texts
        .split_first()
        .expect("a source-map fixture parses at least one source");
    let mut syntax = syntax_program_with_id(*first_id, first);
    let mut program = *first;
    for (source_id, text) in rest {
        let forest = syntax_program_with_id(*source_id, text);
        syntax.extend_from(&forest);
        program = text;
    }
    resolved_forest(sources, &syntax, program)
}

/// Lex `text` and parse it under `source_id`, stopping before resolution. The
/// seeded-continuation fixtures resolve their extension forest through
/// `resolve_extension` against a retained base rather than through `resolve`,
/// which is not an ordinary front-end stage, so they take the parsed forest
/// from here and sequence the rest themselves.
pub(crate) fn syntax_program_with_id(source_id: SourceId, text: &str) -> SyntaxTrees {
    let tokens = stage("tokenize", text, Lexer::new(text).tokenize());
    stage(
        "parse",
        text,
        parse_syntax_trees_with_id(source_id, &tokens),
    )
}

/// Lex and parse each text into one forest, returning it with the last text.
fn parsed_forest<'a>(texts: &[(SourceId, &'a str)]) -> (SyntaxTrees, &'a str) {
    let ((first_id, first), rest) = texts
        .split_first()
        .expect("a source-map fixture parses at least one source");
    let mut syntax = syntax_program_with_id(*first_id, first);
    let mut program = *first;
    for (source_id, text) in rest {
        let tokens = stage("tokenize", text, Lexer::new(text).tokenize());
        stage(
            "parse",
            text,
            parse_syntax_trees_into_with_id(&mut syntax, *source_id, &tokens),
        );
        program = text;
    }
    (syntax, program)
}

/// Resolve `syntax` against `sources`.
fn resolved_forest(sources: SourceMap, syntax: &SyntaxTrees, program: &str) -> SymbolResolvedTrees {
    stage(
        "resolve",
        program,
        resolve(ResolutionRequest {
            syntax,
            sources: Some(Arc::new(sources)),
            top_level_bindings: Vec::new(),
        }),
    )
}
