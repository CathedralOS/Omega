//! Parsed top-level `let`/`boundary let` declarations must refuse explicitly
//! at resolution until the PROOF-CONTRACT-MIGRATION lowering leg lands — never
//! silently drop.

use crate::resolution::ResolutionRequest;
use source_files_to_tokens::Lexer;
use tokens_to_syntax_trees::parse_syntax_trees;

fn resolve_error(source: &str) -> Vec<diagnostics::Diagnostic> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    crate::resolve(ResolutionRequest::new(&syntax)).expect_err("resolve must refuse")
}

#[test]
fn top_level_let_refuses_at_resolution() {
    let diagnostics = resolve_error(
        "let greater_than(limit: i32, value: i32): core::Strict<0> =\n    value > limit;",
    );
    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message
            .contains("mathematical declarations are parsed but"),
        "directed refusal: {}",
        diagnostics[0].message
    );
}

#[test]
fn boundary_let_refuses_at_resolution() {
    let diagnostics = resolve_error(
        "boundary let choose<u: core::Level, A: core::Type<u>>(inhabited: core::Squash<A>): A;",
    );
    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message
            .contains("mathematical declarations are parsed but"),
        "directed refusal: {}",
        diagnostics[0].message
    );
}
