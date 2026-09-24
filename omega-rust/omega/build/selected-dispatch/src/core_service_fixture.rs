//! A resident toolchain `core/binding.omg` for crate tests whose fixture
//! sources spell `Binding<R>` carriers. Boundary dispatch and ProgramEntry
//! service custody both classify carriers against the exact core
//! declaration, so they share this one pipeline rather than each rebuilding
//! it.

use std::sync::Arc;
use typed_trees::TypedTrees;

/// The toolchain core service declaration, resident so fixture sources can
/// spell `Binding<R>` against the real core declaration. These bare pipelines
/// build a `SourceMap` with no package scope, so `use
/// omega::language::core::binding` cannot resolve; installing the source with
/// `SourceOrigin::Toolchain` gives the typed-trees service classifier the exact
/// identity it requires.
const CORE_SERVICE_OMG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../source/library/core/binding.omg"
));

/// Type `source` with the core `Service` declaration resident as a Toolchain
/// source. Fixtures spelling `Binding<R>` carriers must declare the closed-over
/// requirement trait `pub`.
pub(crate) fn typed_with_core_service(name: &str, source: &str) -> TypedTrees {
    let mut sources = source::SourceMap::default();
    let service_source_id = sources
        .add_with_metadata(
            std::path::PathBuf::from("source/library/core/binding.omg"),
            CORE_SERVICE_OMG.to_owned(),
            std::path::PathBuf::from("source/library/core"),
            None,
            source::SourceOrigin::Toolchain,
        )
        .source_id;
    let fixture_source_id = sources
        .add(std::path::PathBuf::from(name), source.to_owned())
        .source_id;
    let service_tokens = source_files_to_tokens::Lexer::new(CORE_SERVICE_OMG)
        .tokenize()
        .expect("tokenize core service declaration");
    let mut syntax =
        tokens_to_syntax_trees::parse_syntax_trees_with_id(service_source_id, &service_tokens)
            .expect("parse core service declaration");
    let fixture_tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize dispatch fixture");
    tokens_to_syntax_trees::parse_syntax_trees_into_with_id(
        &mut syntax,
        fixture_source_id,
        &fixture_tokens,
    )
    .expect("parse dispatch fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest {
            syntax: &syntax,
            sources: Some(Arc::new(sources)),
            top_level_bindings: Vec::new(),
        },
    )
    .expect("resolve dispatch fixture");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type dispatch fixture")
}
