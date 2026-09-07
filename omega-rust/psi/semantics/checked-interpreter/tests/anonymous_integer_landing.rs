use checked_interpreter::interpret_entry;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

fn execute(source: &str) -> checked_interpreter::InterpretOutcome {
    let tokens = Lexer::new(source).tokenize().expect("numeric tokens");
    let syntax = parse_syntax_trees(&tokens).expect("numeric syntax");
    let resolved = lower_syntax_trees(&syntax).expect("numeric symbols");
    let typed = lower_symbol_resolved_trees(&resolved).expect("numeric types");
    let checked =
        lower_typed_trees(typed).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    interpret_entry(&checked, "main", &[])
}

#[test]
fn integer_execution_consumes_exact_decimal_landings() {
    for (expression, expected) in [
        ("7 / 2.0 * 2", 7),
        ("0.1 + 0.9", 1),
        ("9007199254740993.0 - 9007199254740992", 1),
        ("7i32 / 2 * 2", 6),
    ] {
        for body in [
            expression.to_owned(),
            format!("let saved: i32 = {expression}; saved"),
            format!("let mut saved: i32 = 0; saved = {expression}; saved"),
            format!("accept({expression})"),
            format!("({expression}) as i32"),
        ] {
            let source = format!(
                "machine accept(value: i32) -> i32 {{ value }}
                 machine main() -> i32 {{ {body} }}"
            );
            let outcome = execute(&source);
            assert_eq!(outcome.error, None, "{source}");
            assert_eq!(outcome.exit_code, expected, "{source}");
        }
    }
}

#[test]
fn integer_peers_and_state_edges_share_exact_decimal_values() {
    for expression in [
        "input * (9007199254740993.0 - 9007199254740992)",
        "(0.1 + 0.9) * input",
    ] {
        let source = format!(
            "machine combine(input: i32 [0..=1]) -> i32 {{ {expression} }}
             machine main() -> i32 {{ combine(1) }}"
        );
        let outcome = execute(&source);
        assert_eq!(outcome.error, None, "{source}");
        assert_eq!(outcome.exit_code, 1, "{source}");
    }
    for source in [
        "machine main() -> i32 { transition true { true -> 7 / 2.0 * 2 false -> 0 } }",
        "machine main() -> i32 { transition { _ -> finish(7 / 2.0 * 2) } state finish(value: i32) -> i32 { value } }",
    ] {
        let outcome = execute(source);
        assert_eq!(outcome.error, None, "{source}");
        assert_eq!(outcome.exit_code, 7, "{source}");
    }
}

#[test]
fn exact_argument_landing_preserves_forwarded_mutable_integer_references() {
    let outcome = execute(
        "machine assign(value: &mut i32) { value = 7 / 2.0 * 2; }
         machine forward(value: &mut i32) { assign(value); }
         machine main() -> i32 { let mut value: i32 = 0; forward(&mut value); value }",
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}

#[test]
fn exact_landing_retains_the_source_expression_step_schedule() {
    let anonymous = execute("machine main() -> i32 { 7 / 2.0 * 2 }");
    let typed = execute("machine main() -> i32 { 7i32 / 2 * 2 }");
    assert_eq!(anonymous.error, None);
    assert_eq!(typed.error, None);
    assert_eq!(anonymous.exit_code, 7);
    assert_eq!(typed.exit_code, 6);
    assert_eq!(anonymous.usage.fuel_units(), typed.usage.fuel_units());
}
