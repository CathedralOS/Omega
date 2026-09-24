//! The front-end pipelines the lowering tests run.
//!
//! A fixture names the pipeline it exercises by calling one of these functions
//! instead of re-sequencing lexing, parsing, resolution, typing and checking by
//! hand, so a reader sees which stages ran before `lower_machine` and a test
//! cannot silently skip one. Every function's doc comment states its exact
//! stage sequence; a stage that fails panics with the stage name and the source
//! it ran on.
//!
//! One file serves both test trees. The `suite` integration target includes it
//! as `mod front_end` (`tests/suite.rs`), and `src/lib.rs` includes the same
//! file under `#[cfg(test)]`, so unit tests and integration tests spell every
//! call `crate::front_end::..` and read one definition of the sequence.

// Both test trees include this one file, so each target sees the pipelines the
// other one calls.
#![allow(dead_code)]

use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use source::{SourceId, SourceMap};
use source_files_to_tokens::Lexer;
use std::sync::Arc;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees::SyntaxTrees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::{
    parse_syntax_trees, parse_syntax_trees_into_with_id, parse_syntax_trees_with_id,
};
use typed_trees::TypedTrees;
use typed_trees_to_checked_trees::{CheckingRequest, lower_typed_trees};

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
/// returned, for the fixture sweeps that skip a source the front end refuses.
pub(crate) fn typed_program_result(source: &str) -> Result<TypedTrees, Vec<Diagnostic>> {
    let tokens = stage("tokenize", source, Lexer::new(source).tokenize());
    let syntax = stage("parse", source, parse_syntax_trees(&tokens));
    let resolved = resolve(ResolutionRequest::new(&syntax))?;
    lower_symbol_resolved_trees(&resolved).map_err(|diagnostic| vec![diagnostic])
}

/// [`typed_program`], then check the typed trees in the settled mode
/// (`lower_typed_trees(.., &CheckingRequest::settled())`).
pub(crate) fn checked_program(source: &str) -> CheckedTrees {
    stage("check", source, checked_program_result(source))
}

/// [`typed_program`], then check the typed trees in the settled mode
/// (`lower_typed_trees(.., &CheckingRequest::settled())`), returning the
/// checker's rejection instead of panicking on it.
pub(crate) fn checked_program_result(source: &str) -> Result<CheckedTrees, Vec<Diagnostic>> {
    lower_typed_trees(typed_program(source), &CheckingRequest::settled())
}

/// Lex and parse `source` as one anonymous source with no source map, let
/// `edit` rewrite the parsed forest, then resolve, type and check the result
/// in the settled mode (`lower_typed_trees(.., &CheckingRequest::settled())`).
/// The edit is the fixture's own syntax rewrite, not a front-end stage.
pub(crate) fn checked_program_with_syntax_edit(
    source: &str,
    edit: impl FnOnce(&mut SyntaxTrees),
) -> CheckedTrees {
    let tokens = stage("tokenize", source, Lexer::new(source).tokenize());
    let mut syntax = stage("parse", source, parse_syntax_trees(&tokens));
    edit(&mut syntax);
    let resolved = stage("resolve", source, resolve(ResolutionRequest::new(&syntax)));
    let typed = stage("type", source, lower_symbol_resolved_trees(&resolved));
    stage(
        "check",
        source,
        lower_typed_trees(typed, &CheckingRequest::settled()),
    )
}

/// Lex and parse each `(source_id, text)` in order into one syntax forest,
/// resolve it against `sources`, then type. The fixture registers its sources
/// (paths, roots, packages, origins) in the map itself; the sequence is what
/// is shared. The last text is the fixture's own program and names the source
/// in resolution, typing and checking failures.
pub(crate) fn typed_program_from_source_map(
    sources: SourceMap,
    texts: &[(SourceId, &str)],
) -> TypedTrees {
    let (syntax, program) = parsed_forest(texts);
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

/// [`typed_program_from_source_map`], then check the typed trees in the
/// settled mode (`lower_typed_trees(.., &CheckingRequest::settled())`).
pub(crate) fn checked_program_from_source_map(
    sources: SourceMap,
    texts: &[(SourceId, &str)],
) -> CheckedTrees {
    let program = texts
        .last()
        .expect("a source-map fixture parses at least one source")
        .1;
    let typed = typed_program_from_source_map(sources, texts);
    stage(
        "check",
        program,
        lower_typed_trees(typed, &CheckingRequest::settled()),
    )
}

/// Lex and parse each text into one forest, returning it with the last text.
fn parsed_forest<'a>(texts: &[(SourceId, &'a str)]) -> (SyntaxTrees, &'a str) {
    let ((first_id, first), rest) = texts
        .split_first()
        .expect("a source-map fixture parses at least one source");
    let tokens = stage("tokenize", first, Lexer::new(first).tokenize());
    let mut syntax = stage(
        "parse",
        first,
        parse_syntax_trees_with_id(*first_id, &tokens),
    );
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
