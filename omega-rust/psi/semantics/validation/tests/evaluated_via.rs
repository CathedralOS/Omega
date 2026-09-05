use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;
use validation::validate_program;

fn validate(source: &str) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax)?;
    let typed = lower_symbol_resolved_trees(&resolved).map_err(|diagnostic| vec![diagnostic])?;
    validate_program(&typed)
}

#[test]
fn admits_one_exact_zero_argument_via_producer_call_shape() {
    validate(
        r#"
        boundary trait Console {
            machine write(value: u8);
        }

        machine binding() -> i32 {
            0
        }

        machine write_leaf(value: u8)
        satisfies Console::write
        via binding();
        "#,
    )
    .expect("the ordinary via carrier admits one exact zero-argument producer call");
}

#[test]
fn rejects_via_producer_call_with_runtime_arguments() {
    let diagnostics = validate(
        r#"
        boundary trait Console {
            machine write(value: u8);
        }

        machine binding(selector: i32) -> i32 {
            selector
        }

        machine write_leaf(value: u8)
        satisfies Console::write
        via binding(1);
        "#,
    )
    .expect_err("the first ordinary via rung has no runtime argument lane");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("one exact receiverless zero-argument machine call")
    }));
}

#[test]
fn rejects_non_call_via_expression() {
    let diagnostics = validate(
        r#"
        boundary trait Console {
            machine write(value: u8);
        }

        machine write_leaf(value: u8)
        satisfies Console::write
        via 1;
        "#,
    )
    .expect_err("a locator value must come from one exact producer closure");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("one direct zero-argument machine call")
    }));
}

#[test]
fn rejects_parameterized_via_producer() {
    let diagnostics = validate(
        r#"
        boundary trait Console {
            machine write(value: u8);
        }

        machine binding(selector: i32) -> i32 {
            selector
        }

        machine write_leaf(value: u8)
        satisfies Console::write
        via binding();
        "#,
    )
    .expect_err("the first ordinary via rung requires a zero-parameter producer");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "the first evaluated-binding rung requires its body-bearing, non-generic, zero-parameter entry state",
        )
    }));
}

#[test]
fn rejects_bodyless_via_producer() {
    let diagnostics = validate(
        r#"
        boundary trait Console {
            machine write(value: u8);
        }

        boundary machine binding() -> i32;

        machine write_leaf(value: u8)
        satisfies Console::write
        via binding();
        "#,
    )
    .expect_err("the first ordinary via rung requires a body-bearing producer");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "the first evaluated-binding rung requires its body-bearing, non-generic, zero-parameter entry state",
        )
    }));
}
