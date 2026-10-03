use checked_interpreter::BuildMachineEntry;
use checked_interpreter::InterpretOptions;
use checked_interpreter::{InterpretOutcome, interpret_entry};

fn checked(source: &str) -> checked_trees::CheckedTrees {
    try_checked(source).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics}"))
}

fn try_checked(source: &str) -> Result<checked_trees::CheckedTrees, String> {
    crate::front_end::checked_program_result(source)
        .map_err(|diagnostics| format!("{diagnostics:#?}"))
}

fn execute(source: &str) -> InterpretOutcome {
    interpret_entry(
        &checked(source),
        BuildMachineEntry::Name("main"),
        &[],
        InterpretOptions::default(),
    )
}

fn assert_seven(source: &str) {
    let outcome = execute(source);
    assert_eq!(outcome.error, None, "{source}");
    assert_eq!(outcome.exit_code, 7, "{source}");
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
            let outcome = interpret_entry(
                &changed,
                BuildMachineEntry::Name("main"),
                &[],
                InterpretOptions::default(),
            );
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

#[test]
fn inline_const_generic_selectors_execute_distinct_inferred_extents() {
    // The generic body must establish its promised endpoint bound for every
    // admitted N, not just the two concrete applications exercised below.
    for (before_endpoint, after_endpoint, pair_length, triple_length) in
        [("..", "", 2, 3), ("", "..", 2, 1), ("..=", "", 3, 4)]
    {
        assert_seven(&format!("data Main {{}}
            machine Main::endpoint<const N: u64>(witness: &[u8; N]) -> u64
            requires N <= 3
            ensures result <= 3
            {{
                N
            }}
            machine Main::window(items: &[i32; 4]) -> i32 {{
                let pair: [u8; 2] = [0, 0];
                let triple: [u8; 3] = [0, 0, 0];
                let pair_view: &[i32] = items[{before_endpoint}Main::endpoint(&pair){after_endpoint}];
                let triple_view: &[i32] = items[{before_endpoint}Main::endpoint(&triple){after_endpoint}];
                transition pair_view.len == {pair_length} && triple_view.len == {triple_length} {{ true -> 7 false -> 0 }}
            }}
            machine main() -> i32 {{
                let values: [i32; 4] = [11, 7, 6, 22];
                Main::window(&values)
            }}"));
    }
}

#[test]
fn selector_result_contracts_must_establish_the_actual_window_bounds() {
    for (carrier, contract, returned) in [
        ("u64", "ensures result <= 5", "5"),
        ("u64", "", "2"),
        ("i32", "ensures result <= 3", "-1"),
    ] {
        let source = format!(
            "machine endpoint() -> {carrier} {contract} {{ {returned} }}
             machine main() -> u64 {{
                 let values: [u8; 4] = [0, 0, 0, 0];
                 let view: &[u8] = &values[..endpoint()];
                 view.len
             }}"
        );
        let diagnostics = try_checked(&source).expect_err("unproven window must reject");
        assert!(
            diagnostics.contains("subslice range"),
            "{source}: {diagnostics}"
        );
    }
}

#[test]
fn inferred_endpoint_extent_must_satisfy_the_generic_requirement() {
    let diagnostics = try_checked(
        "data Main {}
         machine Main::endpoint<const N: u64>(witness: &[u8; N]) -> u64
         requires N <= 3
         { N }
         machine main() -> u64 {
             let oversized: [u8; 4] = [0, 0, 0, 0];
             Main::endpoint(&oversized)
         }",
    )
    .expect_err("inference cannot discard the endpoint's declared requirement");
    assert!(
        diagnostics.contains("required fact `N <= 3`"),
        "{diagnostics}"
    );
    assert!(diagnostics.contains("4 <= 3"), "{diagnostics}");
}
