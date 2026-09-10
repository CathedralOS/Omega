use super::*;

fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize operator crash fixture");
    let syntax = parse_syntax_trees(&tokens).expect("parse operator crash fixture");
    let resolved = lower_syntax_trees(&syntax).expect("resolve operator crash fixture");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type operator crash fixture");
    lower_typed_trees(typed)
}

#[test]
fn selected_expression_operator_cannot_drop_its_crash_contract() {
    for format in ["i32", "f32", "f64"] {
        for (operator_contract, caller_contract) in [
            ("crashes Trap", ""),
            ("crashes Abort", ""),
            ("crashes Trap", "crashes Abort"),
            ("crashes Abort", "crashes Trap"),
            ("crashes Trap", "crashes Trap"),
            ("crashes Trap false", ""),
        ] {
            let source = format!(
                "boundary operator == Comparison::equal(left: {format}, right: {format}) -> bool {operator_contract};
                 machine compare(left: {format}, right: {format}) -> bool {caller_contract} {{ left == right }}"
            );
            let diagnostics = check(&source).err().unwrap_or_else(|| {
                panic!("selected operator crash contract disappeared: {source}")
            });
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
                    "selected operator with a crashes contract requires invocation-specific crash support"
                )),
                "{source}\n{diagnostics:#?}"
            );
        }
    }
}

#[test]
fn crash_free_expression_operator_remains_accepted() {
    for format in ["i32", "f32", "f64"] {
        check(&format!(
            "boundary operator == Comparison::equal(left: {format}, right: {format}) -> bool;
             machine compare(left: {format}, right: {format}) -> bool {{ left == right }}"
        ))
        .expect("a crash-free selected comparison owes no crash invocation");
    }
}

#[test]
fn unselected_crash_qualified_overload_does_not_create_an_invocation() {
    check(
        "boundary operator == Float::equal(left: f32, right: f32) -> bool crashes Trap;
         boundary operator == Float::equal(left: f64, right: f64) -> bool;
         machine compare(left: f64, right: f64) -> bool { left == right }
         machine integer_compare(left: u64, right: u64) -> bool { left == right }",
    )
    .expect("neither the f64 overload nor builtin equality selects the f32 crash contract");
}
