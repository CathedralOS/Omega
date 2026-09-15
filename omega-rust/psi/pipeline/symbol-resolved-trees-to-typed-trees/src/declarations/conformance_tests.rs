use crate::lower_symbol_resolved_trees;
use source_files_to_tokens::Lexer;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;

#[test]
fn conformance_lifetimes_use_the_declared_binder_and_reject_out_of_scope_names() {
    let source = r#"
        trait Converter<'view, Source, Target> {}
        GenericConversion<'scope, Source, const Width: u64, machine Convert>:
            Source satisfies Converter<'scope, Source, u64>
        where machine Convert(value: Source) -> u64;
        {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokens");
    let syntax = parse_syntax_trees(&tokens).expect("syntax");
    let mut resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolution");
    let typed = lower_symbol_resolved_trees(&resolved).expect("valid conformance");
    assert_eq!(typed.conformances()[0].trait_lifetime_arguments, vec![0]);

    resolved.conformances[0].trait_lifetime_arguments[0] =
        symbol_resolved_trees::name::DiagnosticName::generated_static("outside");
    let error = lower_symbol_resolved_trees(&resolved).expect_err("out-of-scope lifetime");
    let message = format!("{error:?}");
    assert!(message.contains("GenericConversion"), "{message}");
    assert!(
        message.contains("does not name an in-scope conformance lifetime binder"),
        "{message}"
    );
}
