//! The front-end pipelines the checking tests run.
//!
//! A fixture names the pipeline it exercises by calling one of these functions
//! instead of re-sequencing lexing, parsing, resolution and typing by hand, so
//! a reader sees which stages ran and which checking mode consumed the result,
//! and a test cannot silently skip a stage. Every function's doc comment
//! states its exact stage sequence; a stage that fails panics with the stage
//! name and the source it ran on.

use crate::{CheckingRequest, lower_typed_trees};
use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use source::{SourceId, SourceMap, SourceOrigin};
use source_files_to_tokens::Lexer;
use std::path::PathBuf;
use std::sync::Arc;
use symbol_resolved_trees::SymbolResolvedTrees;
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

/// Lex, parse and resolve `source` as one anonymous source with no source map;
/// lexing and parsing panic, resolution's rejection is returned.
fn resolved_program_result(source: &str) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
    let tokens = stage("tokenize", source, Lexer::new(source).tokenize());
    let syntax = stage("parse", source, parse_syntax_trees(&tokens));
    resolve(ResolutionRequest::new(&syntax))
}

/// Lex, parse and resolve `source` as one anonymous source with no source
/// map, for tests that assert on the resolved trees before typing.
pub(crate) fn resolved_program(source: &str) -> SymbolResolvedTrees {
    stage("resolve", source, resolved_program_result(source))
}

/// Lex, parse, resolve and type `source` as one anonymous source with no
/// source map.
pub(crate) fn typed_program(source: &str) -> TypedTrees {
    typed_program_with_resolution(source).1
}

/// Lex, parse, resolve and type `source` as one anonymous source with no
/// source map; lexing and parsing panic, a resolution or typing rejection is
/// returned.
pub(crate) fn typed_program_result(source: &str) -> Result<TypedTrees, Vec<Diagnostic>> {
    let resolved = resolved_program_result(source)?;
    lower_symbol_resolved_trees(&resolved).map_err(|diagnostic| vec![diagnostic])
}

/// [`typed_program`], also handing back the resolved trees typing consumed,
/// for tests that assert on resolution before checking.
pub(crate) fn typed_program_with_resolution(source: &str) -> (SymbolResolvedTrees, TypedTrees) {
    let resolved = resolved_program(source);
    let typed = stage("type", source, lower_symbol_resolved_trees(&resolved));
    (resolved, typed)
}

/// Lex, parse, normalize generic data (`pre_resolution::normalize_generic_data`),
/// resolve and type `source` as one anonymous source with no source map;
/// lexing and parsing panic, a normalization, resolution or typing rejection
/// is returned.
pub(crate) fn typed_program_with_generic_data_result(
    source: &str,
) -> Result<TypedTrees, Vec<Diagnostic>> {
    let tokens = stage("tokenize", source, Lexer::new(source).tokenize());
    let syntax = stage("parse", source, parse_syntax_trees(&tokens));
    let syntax = normalize_generic_data(GenericDataRequest::new(syntax))?;
    let resolved = resolve(ResolutionRequest::new(&syntax))?;
    lower_symbol_resolved_trees(&resolved).map_err(|diagnostic| vec![diagnostic])
}

/// Lex and parse each `(source_id, text)` in order into one syntax forest,
/// resolve it against `sources`, then type. The fixture registers its sources
/// (origins, roots, packages) in the map itself; the last text is the
/// fixture's own program and names the source in resolution and typing
/// failures.
pub(crate) fn typed_program_from_source_map(
    sources: SourceMap,
    texts: &[(SourceId, &str)],
) -> TypedTrees {
    let (syntax, program) = parsed_forest(texts);
    typed_forest(sources, syntax, program)
}

/// [`typed_program_from_source_map`] with generic data normalized
/// (`pre_resolution::normalize_generic_data`) between parsing and
/// resolution.
pub(crate) fn typed_program_from_source_map_with_generic_data(
    sources: SourceMap,
    texts: &[(SourceId, &str)],
) -> TypedTrees {
    let (syntax, program) = parsed_forest(texts);
    let syntax = stage(
        "normalize generic data",
        program,
        normalize_generic_data(GenericDataRequest::new(syntax)),
    );
    typed_forest(sources, syntax, program)
}

/// Lex and parse each text into one forest, returning it with the last text.
fn parsed_forest<'a>(texts: &[(SourceId, &'a str)]) -> (syntax_trees::SyntaxTrees, &'a str) {
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

/// Resolve `syntax` against `sources`, then type.
fn typed_forest(
    sources: SourceMap,
    syntax: syntax_trees::SyntaxTrees,
    program: &str,
) -> TypedTrees {
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

/// The toolchain core service declaration, resident so raw-pipeline fixtures
/// can spell `Binding<R>` against the real core declaration. These unit
/// harnesses build a bare `SourceMap` with no package scope, so `use
/// omega::language::core::service` cannot resolve; installing the source with
/// `SourceOrigin::Toolchain` gives the typed-trees service classifier the
/// exact identity it requires.
const CORE_SERVICE_OMG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../source/library/core/service.omg"
));

/// [`typed_program_from_source_map`] over `core/service.omg` (registered as a
/// Toolchain source) followed by `source` (registered as `tests/main.omg`).
/// Fixtures exercising service-carrier semantics spell `Binding<R>` fields
/// and parameters; the requirement trait they close over must be `pub`.
pub(crate) fn typed_program_with_core_service(source: &str) -> TypedTrees {
    let mut sources = SourceMap::default();
    let service_source_id = sources
        .add_with_metadata(
            PathBuf::from("source/library/core/service.omg"),
            CORE_SERVICE_OMG.to_owned(),
            PathBuf::from("source/library/core"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let user_source_id = sources
        .add(PathBuf::from("tests/main.omg"), source.to_owned())
        .source_id;
    typed_program_from_source_map(
        sources,
        &[
            (service_source_id, CORE_SERVICE_OMG),
            (user_source_id, source),
        ],
    )
}

/// [`typed_program`], then check the typed trees in the settled mode
/// (`lower_typed_trees(.., &CheckingRequest::settled())`).
pub(crate) fn checked_program(source: &str) -> CheckedTrees {
    stage("check", source, checked_program_result(source))
}

/// [`typed_program`], then check the typed trees in the settled mode
/// (`lower_typed_trees(.., &CheckingRequest::settled())`), returning the
/// checker's rejection.
pub(crate) fn checked_program_result(source: &str) -> Result<CheckedTrees, Vec<Diagnostic>> {
    lower_typed_trees(typed_program(source), &CheckingRequest::settled())
}
