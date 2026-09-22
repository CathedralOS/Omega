//! The front-end pipelines this crate's differential targets run.
//!
//! A fixture names the pipeline it exercises by calling one of these functions
//! instead of re-sequencing lexing, parsing, resolution, typing and checking by
//! hand, so a reader sees which stages ran before Terminal production, native
//! realization or an expected refusal, and a target cannot silently skip one.
//! Every function's doc comment states its exact stage sequence.
//!
//! A stage that fails panics with the stage name, the fixture's subject and the
//! source it ran on. The subject is part of the contract, not decoration:
//! `scalar_case_results`'s unproved-requirement control is a `should_panic`
//! test that pins the stage which must refuse it by name.

// Shared across test targets through `#[path]`; each target uses the subset
// its fixtures need.
#![allow(dead_code)]

use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::TypedTrees;
use typed_trees_to_checked_trees::{CheckingRequest, lower_typed_trees};

/// Unwrap one stage's result, or panic with the stage name, the fixture's
/// subject and the source it ran on.
fn stage<T, E: std::fmt::Debug>(
    name: &str,
    subject: &str,
    source: &str,
    result: Result<T, E>,
) -> T {
    result.unwrap_or_else(|error| panic!("{name} {subject}: {error:#?}\nsource:\n{source}"))
}

/// Lex, parse, resolve, type and check `source` as one anonymous source with
/// no source map, in the settled checking mode
/// (`lower_typed_trees(.., &CheckingRequest::settled())`). Every stage panics.
pub(crate) fn checked_program(source: &str) -> CheckedTrees {
    checked_program_named("source", source)
}

/// [`checked_program`], with `subject` naming the fixture in every stage
/// panic, for a target whose own control asserts which stage refused it.
pub(crate) fn checked_program_named(subject: &str, source: &str) -> CheckedTrees {
    let typed = typed_program_named(subject, source);
    stage(
        "check",
        subject,
        source,
        lower_typed_trees(typed, &CheckingRequest::settled()),
    )
}

/// Lex, parse, resolve, type and check `source` as one anonymous source with
/// no source map, in the settled checking mode; lexing, parsing, resolution
/// and typing panic, and the checker's rejection is returned, for the controls
/// whose subject is that refusal.
pub(crate) fn checked_program_result(source: &str) -> Result<CheckedTrees, Vec<Diagnostic>> {
    lower_typed_trees(
        typed_program_named("source", source),
        &CheckingRequest::settled(),
    )
}

/// Lex, parse, resolve and type `source` as one anonymous source with no
/// source map, with `subject` naming the fixture in every stage panic.
fn typed_program_named(subject: &str, source: &str) -> TypedTrees {
    let tokens = stage("tokenize", subject, source, Lexer::new(source).tokenize());
    let syntax = stage("parse", subject, source, parse_syntax_trees(&tokens));
    let resolved = stage(
        "resolve",
        subject,
        source,
        resolve(ResolutionRequest::new(&syntax)),
    );
    stage(
        "type",
        subject,
        source,
        lower_symbol_resolved_trees(&resolved),
    )
}
