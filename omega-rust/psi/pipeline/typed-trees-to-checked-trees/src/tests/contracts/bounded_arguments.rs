use super::{lower_typed_trees, parse_typed_trees};

fn rejects_range(source: &str) {
    let diagnostics = match lower_typed_trees(parse_typed_trees(source)) {
        Ok(_) => panic!("out-of-range argument was accepted"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("not provably within its declared range")),
        "{diagnostics:#?}"
    );
}

#[test]
fn statement_and_value_calls_must_establish_the_parameter_range() {
    for argument in ["0", "6"] {
        for body in [
            format!("_ = accept({argument});"),
            format!("let result: u32 = accept({argument});"),
        ] {
            rejects_range(&format!(
                "machine accept(value: u32 [1..=5]) -> u32 {{ value }} machine run() {{ {body} }}"
            ));
        }
    }
    for body in ["_ = accept(1);", "let result: u32 = accept(5);"] {
        lower_typed_trees(parse_typed_trees(&format!(
            "machine accept(value: u32 [1..=5]) -> u32 {{ value }} machine run() {{ {body} }}"
        )))
        .expect("in-range call");
    }
}

#[test]
fn incoming_argument_guards_keep_their_own_polarity() {
    let positive = r#"
        machine accept(delivered: u32 [1..=5]) -> u32 { delivered }
        machine run(value: u32 [0..=5]) -> u32 {
            transition value > 0 {
                true -> accept(value)
                false -> 0
            }
        }
    "#;
    lower_typed_trees(parse_typed_trees(positive))
        .expect("the positive guard establishes the floor");
    rejects_range(&positive.replace(
        "true -> accept(value)\n                false -> 0",
        "true -> 0\n                false -> accept(value)",
    ));
}

#[test]
fn named_state_delivery_checks_a_renamed_parameter_range() {
    let source = r#"
        machine run(value: u32 [0..=5]) -> u32 {
            transition value > 0 {
                true -> accept(value)
                false -> 0
            }
            state accept(delivered: u32 [1..=5]) -> u32 { delivered }
        }
    "#;
    lower_typed_trees(parse_typed_trees(source)).expect("guarded named-state arrival");
    rejects_range(&source.replace("value > 0", "value >= 0"));
}

#[test]
fn an_unknown_argument_cannot_claim_the_callees_range() {
    rejects_range(
        r#"
        machine accept(value: u32 [1..=5]) -> u32 { value }
        machine run(source: u32) -> u32 { accept(source) }
    "#,
    );
}

#[test]
fn immutable_singleton_bound_is_not_a_guess_about_a_variable_limit() {
    let source = r#"
        machine accept(value: u32 [1..=4]) -> u32 { value }
        machine run(limit: u32 [5..=5], value: u32 [1..=5]) -> u32 {
            transition value < limit {
                true -> accept(value)
                false -> 0
            }
        }
    "#;
    lower_typed_trees(parse_typed_trees(source)).expect("the immutable limit is exactly five");
    rejects_range(&source.replace("limit: u32 [5..=5]", "limit: u32 [4..=6]"));
}
