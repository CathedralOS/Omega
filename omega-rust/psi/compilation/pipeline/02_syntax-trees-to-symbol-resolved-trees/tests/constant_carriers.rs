use source_files_to_tokens::Lexer;
use symbol_resolved_trees::SymbolResolvedTrees;
use syntax_trees_to_symbol_resolved_trees::ResolutionRequest;
use tokens_to_syntax_trees::parse_syntax_trees;

fn resolve(source: &str) -> Result<SymbolResolvedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize constant carrier");
    let syntax = parse_syntax_trees(&tokens).expect("parse constant carrier");
    syntax_trees_to_symbol_resolved_trees::resolve(ResolutionRequest::new(syntax))
}

#[test]
fn conflicting_initializer_suffix_cannot_be_relanded() {
    for namespace in ["", "module constants;"] {
        for (carrier, initializer) in [("u8", "1u16"), ("f32", "1.5f64")] {
            assert!(resolve(&format!(
                "{namespace} const VALUE: {carrier} = {initializer}; machine value() -> {carrier} {{ VALUE }}"
            )).is_err(), "conflicting {carrier} initializer {initializer}");
        }
    }
}
