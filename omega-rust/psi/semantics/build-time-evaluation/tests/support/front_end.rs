//! The front-end pipelines the build-time-evaluation tests run.
//!
//! A fixture names the pipeline it exercises by calling one of these functions
//! instead of re-sequencing lexing, parsing, resolution and typing by hand, so
//! a reader sees which stages ran before the evaluator under test and a fixture
//! cannot silently skip or add one. Every function's doc comment states its
//! exact stage sequence; a stage that fails panics with the stage name and the
//! source it ran on.
//!
//! Build-time evaluation runs *inside* the front end: `evaluate_pre_resolution`,
//! `const_initializers::evaluate` and the `syntax_probes` resolver all consume a
//! parsed forest and hand one back before ordinary resolution. Those are this
//! crate's own operations, not front-end stages, so the fixtures that drive them
//! take a forest from [`syntax_program`] (or its identified variants) and
//! sequence the rest themselves; the stage under test stays visible at the test.
//! The `typed_program*` pipelines are for the fixtures that only need a typed
//! program to evaluate against.
//!
//! One file serves all three test trees: `src/lib.rs` includes it under
//! `#[cfg(test)]`, and the `build_machines` and `exact_symbol_result`
//! integration targets each include the same file, so every site spells the call
//! `crate::front_end::..` and reads one definition of the sequence.

// Every test tree includes this one file, so each target sees the pipelines the
// others call.
#![allow(dead_code)]

use diagnostics::Diagnostic;
use source::{SourceId, SourceMap};
use source_files_to_tokens::Lexer;
use std::sync::Arc;
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

/// Lex and parse `source` as one anonymous source with no source map, stopping
/// before resolution, for the fixtures that drive a build-time evaluation or a
/// pre-resolution probe over the parsed forest.
pub(crate) fn syntax_program(source: &str) -> SyntaxTrees {
    let tokens = stage("tokenize", source, Lexer::new(source).tokenize());
    stage("parse", source, parse_syntax_trees(&tokens))
}

/// [`syntax_program`], parsing `text` under `source_id` instead of the
/// anonymous default, for the fixtures whose evaluation reads a `SourceMap`.
pub(crate) fn syntax_program_with_id(source_id: SourceId, text: &str) -> SyntaxTrees {
    let tokens = stage("tokenize", text, Lexer::new(text).tokenize());
    stage(
        "parse",
        text,
        parse_syntax_trees_with_id(source_id, &tokens),
    )
}

/// Lex and parse each `(source_id, text)` in order into one syntax forest,
/// stopping before resolution. The forest carries the first text's `source_id`,
/// as `parse_syntax_trees_with_id` gives it. The fixture registers its sources
/// in the map itself; the sequence is what is shared.
pub(crate) fn syntax_program_from_texts(texts: &[(SourceId, &str)]) -> SyntaxTrees {
    let ((first_id, first), rest) = texts
        .split_first()
        .expect("a multi-source fixture parses at least one text");
    let mut syntax = syntax_program_with_id(*first_id, first);
    for (source_id, text) in rest {
        let tokens = stage("tokenize", text, Lexer::new(text).tokenize());
        stage(
            "parse",
            text,
            parse_syntax_trees_into_with_id(&mut syntax, *source_id, &tokens),
        );
    }
    syntax
}

/// Lex, parse, resolve and type `source` as one anonymous source with no source
/// map.
pub(crate) fn typed_program(source: &str) -> TypedTrees {
    let syntax = syntax_program(source);
    let resolved = stage("resolve", source, resolve(ResolutionRequest::new(&syntax)));
    stage("type", source, lower_symbol_resolved_trees(&resolved))
}

/// Lex and parse `source` as one anonymous source with no source map, close its
/// generic data applications with the pre-resolution rewrite
/// (`pre_resolution::normalize_generic_data`), then resolve and type. The
/// rewrite runs ahead of resolution rather than inside it, so the fixtures whose
/// const arguments need canonical instances name it here.
pub(crate) fn typed_program_with_generic_data(source: &str) -> TypedTrees {
    let syntax = syntax_program(source);
    let syntax = stage(
        "normalize generic data",
        source,
        normalize_generic_data(GenericDataRequest::new(syntax)),
    );
    let resolved = stage("resolve", source, resolve(ResolutionRequest::new(&syntax)));
    stage("type", source, lower_symbol_resolved_trees(&resolved))
}

/// Lex and parse each `(source_id, text)` in order into one syntax forest,
/// resolve it against `sources`, then type. The fixture registers its sources
/// (paths, roots, packages, origins) in the map itself; the sequence is what is
/// shared. The last text is the fixture's own program and names the source in
/// resolution and typing failures.
pub(crate) fn typed_program_from_source_map(
    sources: SourceMap,
    texts: &[(SourceId, &str)],
) -> TypedTrees {
    typed_program_from_source_map_with_syntax(sources, texts).1
}

/// [`typed_program_from_source_map`], also handing back the parsed forest
/// resolution consumed, for the fixtures that read the syntax and the typed
/// program in one test.
pub(crate) fn typed_program_from_source_map_with_syntax(
    sources: SourceMap,
    texts: &[(SourceId, &str)],
) -> (SyntaxTrees, TypedTrees) {
    let syntax = syntax_program_from_texts(texts);
    let program = texts
        .last()
        .expect("a source-map fixture parses at least one source")
        .1;
    let resolved = stage(
        "resolve",
        program,
        resolve(ResolutionRequest {
            syntax: &syntax,
            sources: Some(Arc::new(sources)),
            top_level_bindings: Vec::new(),
        }),
    );
    let typed = stage("type", program, lower_symbol_resolved_trees(&resolved));
    (syntax, typed)
}

/// Resolve `syntax` against `sources`, then type it. The fixtures that drive
/// one of this crate's own pre-resolution evaluations (`evaluate_pre_resolution`,
/// `const_initializers::evaluate`, the generic-data rewrite) hold the forest that
/// evaluation produced; this is the rest of the front end running over it.
pub(crate) fn typed_program_from_evaluated_syntax(
    syntax: &SyntaxTrees,
    sources: Option<Arc<SourceMap>>,
) -> TypedTrees {
    forest_stage(
        "type",
        typed_program_from_evaluated_syntax_result(syntax, sources),
    )
}

/// [`typed_program_from_evaluated_syntax`], returning the resolution or typing
/// rejection instead of panicking on it.
pub(crate) fn typed_program_from_evaluated_syntax_result(
    syntax: &SyntaxTrees,
    sources: Option<Arc<SourceMap>>,
) -> Result<TypedTrees, Vec<Diagnostic>> {
    let resolved = resolve(ResolutionRequest {
        syntax,
        sources,
        top_level_bindings: Vec::new(),
    })?;
    lower_symbol_resolved_trees(&resolved).map_err(|diagnostic| vec![diagnostic])
}

/// Unwrap one stage's result over an already-evaluated forest, which has no one
/// source text to print, and panic with the stage name.
fn forest_stage<T, E: std::fmt::Debug>(name: &str, result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| panic!("{name}: {error:#?}"))
}
