use super::*;

fn check(source: &str, accepted: bool, rejection: &str) {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    match lower_typed_trees(typed) {
        Ok(_) => assert!(accepted, "stale entry assumption accepted: {source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(rejection)),
                "expected {rejection}: {diagnostics:#?}"
            );
        }
    }
}

fn mutated_entry(collection: &str, index: &str, mutation: &str) {
    check(
        &format!(
            r#"
                machine set_index(output: &mut u64) {{ output = 255; }}
                machine main(items: &[u64], mut index: u64) -> u64
                requires index < items.len
                {{
                    {mutation}
                    transition {{ _ -> read(items, index) }}
                    state read({collection}: &[u64], {index}: u64) -> u64 {{
                        {collection}[{index}]
                    }}
                }}
            "#
        ),
        false,
        "index",
    );
}

#[test]
fn assignment_invalidates_same_named_state_entry_bounds() {
    mutated_entry("items", "index", "index = 255;");
}

#[test]
fn assignment_invalidates_renamed_state_entry_bounds() {
    mutated_entry("values", "position", "index = 255;");
}

#[test]
fn call_invalidates_same_named_state_entry_bounds() {
    mutated_entry("items", "index", "set_index(&mut index);");
}

#[test]
fn call_invalidates_renamed_state_entry_bounds() {
    mutated_entry("values", "position", "set_index(&mut index);");
}

#[test]
fn explicit_state_bounds_are_available_without_machine_bound_clauses() {
    check(
        r#"
        machine main(values: &[u64; 2], index: u64 [0..=1]) -> u64 {
            transition { _ -> read(values, index) }
            state read(items: &[u64; 2], position: u64) -> u64
            requires position < 2
            { items[position] }
        }
    "#,
        true,
        "",
    );
}

#[test]
fn replacing_a_slice_descriptor_retires_its_old_extent() {
    for (replacement, accepted) in [("other", false), ("items[0..1]", true)] {
        check(
            &format!(
                r#"
            machine main(mut items: &[u64], other: &[u64]) -> u64
            requires items.len > 0
            {{
                items = {replacement};
                transition {{ _ -> read(items) }}
                state read(items: &[u64]) -> u64 {{ items[0] }}
            }}
        "#
            ),
            accepted,
            "index",
        );
    }
}

#[test]
fn explicit_state_bounds_support_indexing_and_remain_arrival_obligations() {
    for (argument, accepted) in [("first", true), ("other", false)] {
        check(
            &format!(
                r#"
            machine main(items: &[u64], first: u64, other: u64) -> u64
            requires first < items.len
            {{
                transition {{ _ -> read(items, {argument}) }}
                state read(values: &[u64], position: u64) -> u64
                requires position < values.len
                {{ values[position] }}
            }}
        "#
            ),
            accepted,
            "requires",
        );
    }
}

#[test]
fn explicit_state_bounds_flow_to_a_later_state_without_name_replay() {
    check(
        r#"
        machine main(items: &[u64], index: u64) -> u64
        requires index < items.len
        {
            transition { _ -> middle(items, index) }
            state middle(values: &[u64], position: u64) -> u64
            requires position < values.len
            {
                transition { _ -> read(values, position) }
            }
            state read(selected: &[u64], offset: u64) -> u64 { selected[offset] }
        }
    "#,
        true,
        "",
    );
}
