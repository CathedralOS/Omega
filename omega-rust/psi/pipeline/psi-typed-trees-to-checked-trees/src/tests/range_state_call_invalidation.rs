use super::*;

fn check(source: &str, accepted: bool) {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    match lower_typed_trees(typed) {
        Ok(_) => assert!(accepted, "stale state argument accepted: {source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("index")),
                "expected an index rejection: {diagnostics:#?}\n{source}"
            );
        }
    }
}

#[test]
fn named_state_parameters_retain_minimum_not_exact_collection_length() {
    for transition in ["read(input)", "forward(input)"] {
        let forwarding = if transition == "forward(input)" {
            "state forward(middle: &[u64]) -> u64 { transition { _ -> read(middle) } }"
        } else {
            ""
        };
        check(
            &format!(
                r#"
            machine main(input: &[u64]) -> u64 {{
                transition input.len > 0 {{
                    true -> {transition}
                    false -> 0
                }}
                {forwarding}
                state read(items: &[u64]) -> u64 {{ items[0] }}
            }}
        "#
            ),
            true,
        );
    }
}

#[test]
fn named_state_minimum_length_meets_every_incoming_edge() {
    for (second_guard, selected, accepted) in [
        ("items.len > 0", 0, true),
        ("items.len > 0", 1, false),
        ("true", 0, false),
    ] {
        check(
            &format!(
                r#"
            machine main(first: &[u64], second: &[u64], choose: bool) -> u64 {{
                transition choose {{
                    true -> left(first)
                    false -> right(second)
                }}
                state left(items: &[u64]) -> u64 {{
                    transition items.len > 1 {{
                        true -> read(items)
                        false -> 0
                    }}
                }}
                state right(items: &[u64]) -> u64 {{
                    transition {second_guard} {{
                        true -> read(items)
                        false -> 0
                    }}
                }}
                state read(selected: &[u64]) -> u64 {{ selected[{selected}] }}
            }}
        "#
            ),
            accepted,
        );
    }
}

#[test]
fn call_written_indices_are_not_transported_to_named_states() {
    for call in [
        "set_index(&mut index);",
        "let ignored: u64 = set_index_value(&mut index);",
        "let mut ignored: u64 = 0; ignored = set_index_value(&mut index);",
    ] {
        check(
            &format!(
                r#"
            machine set_index(index: &mut u64) {{ index = 255; }}
            machine set_index_value(index: &mut u64) -> u64 {{ index = 255; 0 }}
            machine main() -> u64 {{
                let values: [u64; 2] = [10, 20];
                let mut index: u64 = 0;
                {call}
                transition {{ _ -> read(values, index) }}
                state read(values: [u64; 2], index: u64) -> u64 {{ values[index] }}
            }}
        "#
            ),
            false,
        );
    }
}

#[test]
fn readonly_and_disjoint_calls_preserve_named_state_indices() {
    for call in [
        "inspect(index);",
        "set_index(&mut unrelated);",
        "let ignored: u64 = inspect_value(index);",
        "let ignored: u64 = set_index_value(&mut unrelated);",
    ] {
        check(
            &format!(
                r#"
            machine inspect(index: u64) {{}}
            machine inspect_value(index: u64) -> u64 {{ index }}
            machine set_index(index: &mut u64) {{ index = 255; }}
            machine set_index_value(index: &mut u64) -> u64 {{ index = 255; 0 }}
            machine main() -> u64 {{
                let values: [u64; 2] = [10, 20];
                let index: u64 = 0;
                let mut unrelated: u64 = 0;
                {call}
                transition {{ _ -> read(values, index) }}
                state read(values: [u64; 2], index: u64) -> u64 {{ values[index] }}
            }}
        "#
            ),
            true,
        );
    }
}

#[test]
fn calls_retire_collection_relative_facts_before_state_transport() {
    for (values, index) in [("items", "position"), ("values", "index")] {
        for (target, accepted) in [("index", false), ("unrelated", true)] {
            let target = if target == "index" { index } else { target };
            check(
                &format!(
                    r#"
            machine set_index(index: &mut u64) {{ index = 255; }}
            machine main(values: &[u64], index: u64) -> u64 {{
                transition index < values.len {{
                    true -> prepare(values, index)
                    false -> 0
                }}
                state prepare({values}: &[u64], mut {index}: u64) -> u64 {{
                    let mut unrelated: u64 = 0;
                    set_index(&mut {target});
                    transition {{ _ -> read({values}, {index}) }}
                }}
                state read({values}: &[u64], {index}: u64) -> u64 {{ {values}[{index}] }}
            }}
        "#
                ),
                accepted,
            );
        }
    }
}

#[test]
fn stored_mutating_guards_do_not_transport_pre_call_comparisons() {
    check(
        r#"
        machine set_index(index: &mut u64) -> bool { index = 255; true }
        machine main(values: &[u64], mut index: u64) -> u64 {
            let ready: bool = index < values.len && set_index(&mut index);
            transition ready {
                true -> read(values, index)
                false -> 0
            }
            state read(items: &[u64], position: u64) -> u64 { items[position] }
        }
    "#,
        false,
    );
}

#[test]
fn guard_and_argument_calls_retire_facts_before_target_collection() {
    for transition in [
        "transition set_index(&mut index) { true -> read(values, index, 0) false -> 0 }",
        "transition { _ -> read(values, index, set_index_value(&mut index)) }",
        "transition { _ -> read(values, set_index_value(&mut index), index) }",
    ] {
        // Both index parameters are observed, so either argument order exposes
        // stale storage evidence. No effectful call grants a new scalar value.
        check(
            &format!(
                r#"
            machine set_index(index: &mut u64) -> bool {{ index = 255; true }}
            machine set_index_value(index: &mut u64) -> u64 {{ index = 255; 0 }}
            machine main() -> u64 {{
                let values: [u64; 2] = [10, 20];
                let mut index: u64 = 0;
                {transition}
                state read(values: [u64; 2], first: u64, second: u64) -> u64 {{
                    let ignored: u64 = values[first];
                    values[second]
                }}
            }}
        "#
            ),
            false,
        );
    }
}
