use super::*;

#[test]
fn byte_content_entry_routes_reject_unknown_actuals_and_unrelated_guards() {
    for source in [
        BYTE_SEQUENCE_AGGREGATE_EQUALITY_SOURCE.replace(
            "\n        Helper::inspect(left, right);",
            "\n        let mut current: Borrowed = left; Helper::inspect(current, right);",
        ),
        BYTE_SEQUENCE_AGGREGATE_EQUALITY_SOURCE.replace(
            "machine Root::enter(left: Borrowed, right: Borrowed)\n    crashes Abort\n        left == right",
            "machine Root::enter(left: Borrowed, right: Borrowed)\n    crashes Abort\n        left.active",
        ),
    ] {
        let tokens = Lexer::new(&source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let diagnostics = lower_typed_trees(typed).expect_err("entry route does not cover the call");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("call from `Root::enter` to `Helper::inspect`")
                && diagnostic.message.contains("uncovered Abort crash route")
        }), "{diagnostics:?}");
    }
}
