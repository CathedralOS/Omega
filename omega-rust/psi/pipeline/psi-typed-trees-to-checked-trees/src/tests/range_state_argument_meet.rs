use super::*;

fn check(source: &str, accepted: bool) {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    match lower_typed_trees(typed) {
        Ok(_) => assert!(accepted, "unproved incoming edge accepted: {source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("index")),
                "expected index rejection: {diagnostics:#?}\n{source}"
            );
        }
    }
}

#[test]
fn every_reachable_edge_must_prove_the_collection_index_relation() {
    for (left, right, accepted) in [
        ("first", "other", false),
        ("other", "first", false),
        ("other", "other", false),
        ("first", "first", true),
    ] {
        check(
            &format!(
                r#"
            machine main(items: &[u64], first: u64, other: u64, choose: bool) -> u64
            requires first < items.len
            {{
                transition choose {{ true -> read(items, {left}) false -> read(items, {right}) }}
                state read(values: &[u64], index: u64) -> u64 {{ values[index] }}
            }}
        "#
            ),
            accepted,
        );
    }
}

#[test]
fn one_constant_argument_cannot_replace_an_unknown_argument_on_another_edge() {
    for (left, right, accepted) in [
        ("0", "other", false),
        ("other", "0", false),
        ("0", "1", false),
        ("0", "0", true),
    ] {
        check(
            &format!(
                r#"
            data Host {{ values: [u64; 1]; }}
            machine Host::main(&self, other: u64, choose: bool) -> u64 {{
                transition choose {{ true -> read({left}) false -> read({right}) }}
                state read(&self, index: u64) -> u64 {{ self.values[index] }}
            }}
        "#
            ),
            accepted,
        );
    }
}

#[test]
fn collection_extent_meets_unknown_and_differing_incoming_lengths() {
    for (left, right, accepted) in [
        ("view", "items", false),
        ("items", "view", false),
        ("view", "view", true),
        ("view", "longer_view", true),
    ] {
        check(
            &format!(
                r#"
            machine main(items: &[u8], choose: bool) -> u8 {{
                let fixed: [u8; 1] = [0];
                let longer: [u8; 2] = [0, 1];
                let view: &[u8] = fixed.as_slice();
                let longer_view: &[u8] = longer.as_slice();
                transition choose {{ true -> read({left}) false -> read({right}) }}
                state read(values: &[u8]) -> u8 {{ values[0] }}
            }}
        "#
            ),
            accepted,
        );
    }
}

#[test]
fn reachable_recursive_index_proof_requires_a_valid_entry_anchor() {
    for (initial_index, accepted) in [("0", true), ("unknown", false)] {
        check(
            &format!(
                r#"
            machine main(items: &[u64], unknown: u64) -> u64 {{
                transition items.len > 0 {{
                    true -> visit(items, {initial_index})
                    false -> 0
                }}
                state visit(values: &[u64], index: u64) -> u64 {{
                    let value: u64 = values[index];
                    let next: u64 = index + 1;
                    transition next < values.len {{
                        true -> visit(values, next)
                        false -> value
                    }}
                }}
            }}
        "#
            ),
            accepted,
        );
    }
}

#[test]
fn forwarding_and_state_declaration_order_do_not_drop_incoming_evidence() {
    let middle = "state middle(values: &[u64], index: u64) -> u64 { transition { _ -> read(values, index) } }";
    let read = "state read(values: &[u64], index: u64) -> u64 { values[index] }";
    for states in [format!("{middle} {read}"), format!("{read} {middle}")] {
        check(
            &format!(
                r#"
            machine main(items: &[u64], index: u64) -> u64
            {{
                transition index < items.len {{
                    true -> middle(items, index)
                    false -> 0
                }}
                {states}
            }}
        "#
            ),
            true,
        );
    }
}

#[test]
fn propagation_cap_cannot_publish_a_bound_before_a_late_unknown_edge_arrives() {
    let mut states = String::new();
    for position in 0..70 {
        let next = if position == 69 {
            "read".to_owned()
        } else {
            format!("forward_{}", position + 1)
        };
        states.push_str(&format!(
            "state forward_{position}(values: &[u64], index: u64) -> u64 {{ transition {{ _ -> {next}(values, index) }} }}\n"
        ));
    }
    check(
        &format!(
            r#"
        machine main(items: &[u64], first: u64, other: u64, choose: bool) -> u64
        requires first < items.len
        {{
            transition choose {{ true -> read(items, first) false -> forward_0(items, other) }}
            state read(values: &[u64], index: u64) -> u64 {{ values[index] }}
            {states}
        }}
    "#
        ),
        false,
    );
}

#[test]
fn an_unrelated_machine_with_the_same_name_is_not_a_state_incoming_edge() {
    for (foreign_index, state_index, accepted) in [("other", "0", true), ("0", "other", false)] {
        check(
            &format!(
                r#"
            data Helper {{}}
            machine Helper::read(&self, index: u64) -> u64 {{ 0 }}
            data Host {{ values: [u64; 1]; }}
            machine Host::main(&self, helper: Helper, other: u64) -> u64 {{
                let ignored: u64 = helper.read({foreign_index});
                transition {{ _ -> read({state_index}) }}
                state read(&self, index: u64) -> u64 {{ self.values[index] }}
            }}
        "#
            ),
            accepted,
        );
    }
}
