#![forbid(unsafe_code)]

#[cfg(test)]
use source_admission::LoadedSource;
use source_declarations::{declaration_selection, source_selections};
#[cfg(test)]
use source_lexer as lexing;
#[cfg(test)]
use source_parser as parsing;
use source_symbol_table::{source_inputs, source_symbols, source_tables};
use symbol_resolved_trees::source_resolved;
#[cfg(test)]
use syntax_trees_to_symbol_resolved_trees::resolution;
use typed_trees::source_state;
#[cfg(test)]
#[path = "../../../../../../tests/support/frontend_operations.rs"]
mod frontend_fixture;

#[cfg(test)]
fn resolve_fixture(text: &str) -> source_resolved::SymbolResolvedTrees {
    try_resolve_fixture(text).expect("fixture declarations and bodies resolve")
}

#[cfg(test)]
fn try_resolve_fixture(
    text: &str,
) -> Result<source_resolved::SymbolResolvedTrees, Vec<diagnostics::Diagnostic>> {
    let mut files = source_inputs::SourceMap::default();
    let source = files.add("test/main.omg".into(), text.to_owned()).source_id;
    let tokens = frontend_fixture::Lexer::new(text).tokenize().unwrap();
    let syntax = frontend_fixture::parse_syntax_trees_with_id(source, &tokens).unwrap();
    resolution::resolve(resolution::ResolutionRequest {
        syntax,
        sources: Some(std::sync::Arc::new(files)),
        top_level_bindings: Vec::new(),
        retained_base: None,
    })
}

#[cfg(test)]
#[path = "src/data_application_tests.rs"]
mod data_application_tests;
#[path = "src/lib.rs"]
pub mod typing;
