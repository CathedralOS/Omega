//! Fixtures shared by the lowerer tests: lexing, parsing and resolving
//! source programs into typed trees.

mod generated_invocations;
mod machine_contracts;
mod mathematical_declarations;
mod quotients_and_domains;
mod retained_constants;
mod seeded_continuations;
mod seeded_instances;
mod token_bindings;
mod typed_retention;

use super::seeded_continuation::{SeededTypingBase, lower_symbol_resolved_trees_to_seeded_base};
use crate::lowerer::lower_symbol_resolved_trees;
use source::{SourceMap, SourceOrigin, SourceResolutionStratum};
use source_files_to_tokens::Lexer;
use std::path::PathBuf;
use std::sync::Arc;
use syntax_trees_to_symbol_resolved_trees::{
    ExtensionRequest, RebasedSeededSymbolResolvedTrees, ResolutionRequest, resolve,
    resolve_extension,
};
use tokens_to_syntax_trees::{parse_syntax_trees, parse_syntax_trees_with_id};

fn seeded_plain_data_inputs(
    base_source: &str,
    extension_source: &str,
) -> (SeededTypingBase, RebasedSeededSymbolResolvedTrees) {
    let mut base_sources = SourceMap::default();
    let base_id = base_sources
        .add(PathBuf::from("base.omg"), base_source.to_owned())
        .source_id;
    let base_syntax = parse_syntax_trees_with_id(
        base_id,
        &Lexer::new(base_source).tokenize().expect("tokenize base"),
    )
    .expect("parse base");
    let resolved = resolve(ResolutionRequest {
        syntax: &base_syntax,
        sources: Some(Arc::new(base_sources.clone())),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve base");
    let typing_base =
        lower_symbol_resolved_trees_to_seeded_base(resolved).expect("type retained base");
    assert_eq!(typing_base.typed().symbols.source_files().count(), 1);
    let mut sources = base_sources;
    let extension_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from("generated.omg"),
            extension_source.to_owned(),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let extension_syntax = parse_syntax_trees_with_id(
        extension_id,
        &Lexer::new(extension_source)
            .tokenize()
            .expect("tokenize extension"),
    )
    .expect("parse extension");
    let seeded = resolve_extension(ExtensionRequest {
        base: typing_base.resolved_base_for_extension(),
        syntax: &extension_syntax,
        sources: Arc::new(sources),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve seeded extension");
    let rebased = seeded
        .rebase_authored_selections_for_typed_continuation(
            typing_base.typed().authored_declaration_selections(),
        )
        .expect("rebase extension selections");
    assert_eq!(rebased.trees().symbols.source_files().count(), 2);
    (typing_base, rebased)
}

fn seeded_normalized_plain_data_inputs(
    base_source: &str,
    extension_source: &str,
) -> (SeededTypingBase, RebasedSeededSymbolResolvedTrees) {
    let mut base_sources = SourceMap::default();
    let base_id = base_sources
        .add(PathBuf::from("base.omg"), base_source.to_owned())
        .source_id;
    let base_syntax = parse_syntax_trees_with_id(
        base_id,
        &Lexer::new(base_source).tokenize().expect("tokenize base"),
    )
    .expect("parse base");
    let resolved = resolve(ResolutionRequest {
        syntax: &base_syntax,
        sources: Some(Arc::new(base_sources.clone())),
        top_level_bindings: Vec::new(),
    })
    .expect("resolve base");
    let typing_base =
        lower_symbol_resolved_trees_to_seeded_base(resolved).expect("type retained base");
    let mut sources = base_sources;
    let extension_id = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from("generated.omg"),
            extension_source.to_owned(),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let extension_syntax = parse_syntax_trees_with_id(
        extension_id,
        &Lexer::new(extension_source)
            .tokenize()
            .expect("tokenize extension"),
    )
    .expect("parse extension");
    let resolved_base = typing_base.resolved_base_for_extension();
    let sources = Arc::new(sources);
    let extension_syntax =
        syntax_trees_to_symbol_resolved_trees::pre_resolution::normalize_generic_data(
            syntax_trees_to_symbol_resolved_trees::pre_resolution::GenericDataRequest {
                syntax: extension_syntax,
                sources: Some(sources.clone()),
                top_level_bindings: Vec::new(),
                retained_base: Some(&resolved_base),
            },
        )
        .expect("normalize extension unit with its exact retained argument declarations");
    let seeded = resolve_extension(ExtensionRequest {
        base: resolved_base,
        syntax: &extension_syntax,
        sources,
        top_level_bindings: Vec::new(),
    })
    .expect("resolve normalized seeded extension");
    let rebased = seeded
        .rebase_authored_selections_for_typed_continuation(
            typing_base.typed().authored_declaration_selections(),
        )
        .expect("rebase normalized extension selections");
    (typing_base, rebased)
}

fn lower_source(source: &str) -> Result<typed_trees::TypedTrees, diagnostics::Diagnostic> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve source");
    lower_symbol_resolved_trees(&resolved)
}
