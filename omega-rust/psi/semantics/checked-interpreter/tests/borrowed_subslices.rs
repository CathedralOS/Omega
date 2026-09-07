use checked_interpreter::{InterpretOutcome, interpret_entry};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("subslice tokens");
    let syntax = parse_syntax_trees(&tokens).expect("subslice syntax");
    let resolved = lower_syntax_trees(&syntax).expect("subslice symbols");
    let typed = lower_symbol_resolved_trees(&resolved).expect("subslice types");
    lower_typed_trees(typed).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

fn execute(source: &str) -> InterpretOutcome {
    interpret_entry(&checked(source), "main", &[])
}

fn assert_seven(source: &str) {
    let outcome = execute(source);
    assert_eq!(outcome.error, None, "{source}");
    assert_eq!(outcome.exit_code, 7, "{source}");
}

#[test]
fn borrowed_array_window_writes_through_without_changing_neighbors() {
    assert_seven("machine fill(values: &mut [i32]) {
            values[..] = [0.1 * 70, 7i32 / 2 * 2];
        }
        machine main() -> i32 {
            let mut values: [i32; 4] = [11, 0, 0, 22];
            fill(&mut values[1..3]);
            transition values[0] == 11 && values[1] == 7 && values[2] == 6 && values[3] == 22 { true -> 7 false -> 0 }
        }");
}

#[test]
fn nested_and_inclusive_borrowed_windows_retain_their_extent() {
    for selection in ["1..3", "1..=2"] {
        assert_seven(&format!("machine fill(values: &mut [i32]) {{
                values[..] = [18446744073709551616 / 3 * 3 - 18446744073709551609, 6];
            }}
            machine main() -> i32 {{
                let mut values: [i32; 5] = [11, 12, 0, 0, 22];
                fill(&mut values[1..][{selection}]);
                transition values[0] == 11 && values[1] == 12 && values[2] == 7 && values[3] == 6 && values[4] == 22 {{ true -> 7 false -> 0 }}
            }}"));
    }
}

#[test]
fn packed_byte_windows_share_storage_through_forwarded_loans() {
    for selection in ["1..3", "1..=2"] {
        assert_seven(&format!("machine fill(values: &mut [u8]) {{ values[..] = [0.1 * 70, 6]; }}
            machine forward(values: &mut [u8]) {{ fill(values); }}
            machine main() -> i32 {{
                let mut values: [u8; 4] = \"abcd\";
                forward(&mut values[{selection}]);
                transition values[0] == 97 && values[1] == 7 && values[2] == 6 && values[3] == 100 {{ true -> 7 false -> 0 }}
            }}"));
    }
}

#[test]
fn stored_borrowed_windows_keep_backing_storage() {
    assert_seven("machine fill(values: &mut [i32]) { values[..] = [7, 6]; }
        machine main() -> i32 {
            let mut values: [i32; 4] = [11, 0, 0, 22];
            let view: &mut [i32] = &mut values[1..3];
            fill(view);
            transition values[0] == 11 && values[1] == 7 && values[2] == 6 && values[3] == 22 { true -> 7 false -> 0 }
        }");
}

#[test]
fn shared_windows_observe_only_selected_elements_and_length() {
    for (element, initializer, expected) in
        [("i32", "[11, 7, 6, 22]", "7"), ("u8", "\"abcd\"", "98")]
    {
        assert_seven(&format!(
            "machine inspect(values: &[{element}]) -> i32 {{
                transition values.len == 2 {{ true -> 7 false -> 0 }}
            }}
            machine main() -> i32 {{
                let values: [{element}; 4] = {initializer};
                let view: &[{element}] = &values[1..3];
                let length_result: i32 = inspect(view);
                transition view[0] == {expected} && length_result == 7 {{ true -> 7 false -> 0 }}
            }}"
        ));
    }
}

#[test]
fn borrowed_window_count_mismatch_traps_instead_of_writing_neighbors() {
    for (element, initializer) in [("i32", "[11, 0, 0, 22]"), ("u8", "\"abcd\"")] {
        let outcome = execute(&format!(
            "machine fill(values: &mut [{element}]) {{ values[..] = [7]; }}
            machine main() -> i32 {{
                let mut values: [{element}; 4] = {initializer};
                fill(&mut values[1..3]);
                7
            }}"
        ));
        assert!(
            outcome
                .error
                .as_ref()
                .is_some_and(|error| error.contains("different element count")),
            "{outcome:?}"
        );
    }
}

#[test]
fn borrowed_window_execution_rejects_stale_range_proofs() {
    use typed_trees::expression::ExpressionNode;
    for (element, initializer) in [("i32", "[11, 0, 0, 22]"), ("u8", "\"abcd\"")] {
        let program = checked(&format!(
            "machine inspect(values: &[{element}]) -> i32 {{ 7 }}
            machine main() -> i32 {{
                let values: [{element}; 4] = {initializer};
                inspect(&values[1..3])
            }}"
        ));
        let end = program
            .expression_table
            .expression_entries()
            .find_map(|(_, node)| match node {
                ExpressionNode::Range(range) if range.end.is_valid() => Some(range.end),
                _ => None,
            })
            .expect("borrowed range end");
        for end_value in [0, 5] {
            let mut changed = program.clone();
            *changed.typed.expression_table.expression_mut(end) =
                ExpressionNode::Integer(numerics::literals::IntegerLiteral::from_value(end_value));
            let outcome = interpret_entry(&changed, "main", &[]);
            assert!(
                outcome
                    .error
                    .as_ref()
                    .is_some_and(|error| error.contains("subslice range is out of bounds")),
                "{outcome:?}"
            );
        }
    }
}

#[test]
fn checked_window_selectors_execute_once_for_borrows_reads_and_writes() {
    for operation in [
        "fill(&mut values[first(&mut calls)..3]);",
        "let observed: i32 = values[first(&mut calls)..3][0];",
        "values[first(&mut calls)..3][0] = 7;",
        "let observed: i32 = inspect(values[first(&mut calls)..3]);",
        "let length: u64 = values[first(&mut calls)..3].len;",
    ] {
        assert_seven(&format!("machine first(calls: &mut i32 in Wrapping) -> u64 [1..=1] {{
                calls = calls + 1; 1
            }}
            machine fill(values: &mut [i32]) {{ values[..] = [7, 6]; }}
            machine inspect(values: &[i32]) -> i32 {{ 7 }}
            machine main() -> i32 {{
                let mut calls: i32 in Wrapping = 0;
                let mut values: [i32; 4] = [11, 7, 6, 22];
                {operation}
                transition calls == 1 && values[0] == 11 && values[1] == 7 && values[2] == 6 && values[3] == 22 {{ true -> 7 false -> 0 }}
            }}"));
    }
}

#[test]
fn dynamic_and_empty_borrows_use_their_runtime_extent() {
    assert_seven("machine fill(values: &mut [i32]) { values[..] = [7, 6]; }
        machine empty(values: &mut [i32]) { values[..] = []; }
        machine main() -> i32 {
            let mut values: [i32; 4] = [11, 0, 0, 22];
            let mut start: u64 = 1;
            fill(&mut values[start..3]);
            empty(&mut values[4..]);
            transition values[0] == 11 && values[1] == 7 && values[2] == 6 && values[3] == 22 { true -> 7 false -> 0 }
        }");
}

#[test]
fn declared_selector_call_range_admits_and_executes_the_borrow() {
    assert_seven("machine first(calls: &mut i32 in Wrapping) -> u64 [1..=1] {
            calls = calls + 1; 1
        }
        machine fill(values: &mut [i32]) { values[..] = [0.1 * 70, 6]; }
        machine main() -> i32 {
            let mut calls: i32 in Wrapping = 0;
            let mut values: [i32; 4] = [11, 0, 0, 22];
            fill(&mut values[first(&mut calls)..3]);
            transition calls == 1 && values[0] == 11 && values[1] == 7 && values[2] == 6 && values[3] == 22 { true -> 7 false -> 0 }
        }");
}

#[test]
fn independently_bounded_selector_calls_execute_in_source_order() {
    assert_seven("machine first(calls: &mut i32 in Wrapping) -> u64 [1..=1] {
            calls = calls * 10 + 1; 1
        }
        machine last(calls: &mut i32 in Wrapping) -> u64 [3..=3] {
            calls = calls * 10 + 2; 3
        }
        machine fill(values: &mut [u8]) { values[..] = [0.1 * 70, 6]; }
        machine main() -> i32 {
            let mut calls: i32 in Wrapping = 0;
            let mut values: [u8; 4] = \"abcd\";
            fill(&mut values[first(&mut calls)..last(&mut calls)]);
            transition calls == 12 && values[0] == 97 && values[1] == 7 && values[2] == 6 && values[3] == 100 { true -> 7 false -> 0 }
        }");
}
