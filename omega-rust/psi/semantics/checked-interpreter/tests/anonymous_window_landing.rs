use checked_interpreter::interpret_entry;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("window tokens");
    let syntax = parse_syntax_trees(&tokens).expect("window syntax");
    let resolved = lower_syntax_trees(&syntax).expect("window symbols");
    let typed = lower_symbol_resolved_trees(&resolved).expect("window types");
    lower_typed_trees(typed).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

fn execute(source: &str) -> checked_interpreter::InterpretOutcome {
    interpret_entry(&checked(source), "main", &[])
}

#[test]
fn fixed_integer_windows_store_exact_values_without_changing_neighbors() {
    for expression in [
        "7 / 2 * 2",
        "7 / 2.0 * 2",
        "0.1 * 70",
        "18446744073709551616 / 3 * 3 - 18446744073709551609",
    ] {
        for access in ["write", "mut"] {
            let source = format!("machine fill(values: &{access} [i32; 4]) {{ values[1..3] = [{expression}, 7i32 / 2 * 2]; }}
        machine main() -> i32 {{
            let mut values: [i32; 4] = [11, 0, 0, 22];
            fill(&{access} values);
            transition values[0] == 11 && values[1] == 7 && values[2] == 6 && values[3] == 22 {{ true -> 7 false -> 0 }}
        }}");
            assert_seven(&source);
        }
    }
}

fn assert_seven(source: &str) {
    let outcome = execute(source);
    assert_eq!(outcome.error, None, "{source}");
    assert_eq!(outcome.exit_code, 7, "{source}");
}

#[test]
fn record_windows_keep_inclusive_and_immutable_copy_bounds() {
    assert_seven("data Inner { values: [i32; 4]; } data Outer { inner: Inner; }
        machine fill(outer: &write Outer) {
            let first: u64 = 1; let start: u64 = first; let last: u64 = 2;
            outer.inner.values[start..=last] = [0.1 * 70, 7i32 / 2 * 2];
        }
        machine main() -> i32 {
            let mut outer: Outer = Outer { inner: Inner { values: [11, 0, 0, 22] } };
            fill(&write outer);
            transition outer.inner.values[0] == 11 && outer.inner.values[1] == 7 && outer.inner.values[2] == 6 && outer.inner.values[3] == 22 { true -> 7 false -> 0 }
        }");
}

#[test]
fn nested_array_windows_use_each_scalar_element_destination() {
    assert_seven("machine fill(values: &write [[i32; 2]; 3]) {
            values[1..2] = [[7 / 2.0 * 2, 7i32 / 2 * 2]];
        }
        machine main() -> i32 {
            let mut values: [[i32; 2]; 3] = [[11, 12], [0, 0], [21, 22]];
            fill(&write values);
            transition values[0][0] == 11 && values[1][0] == 7 && values[1][1] == 6 && values[2][1] == 22 { true -> 7 false -> 0 }
        }");
}

#[test]
fn window_element_calls_execute_once_in_source_order() {
    assert_seven("machine mark(counter: &mut i32, value: i32) -> i32 { let prior: i32 = counter; counter = value; prior }
        machine fill(values: &write [i32; 5], counter: &mut i32) {
            values[1..4] = [mark(counter, 1), 0.1 * 70, mark(counter, 2)];
        }
        machine main() -> i32 {
            let mut counter: i32 = 0;
            let mut values: [i32; 5] = [11, 0, 0, 0, 22];
            fill(&write values, &mut counter);
            transition values[0] == 11 && values[1] == 0 && values[2] == 7 && values[3] == 1 && values[4] == 22 && counter == 2 { true -> 7 false -> 0 }
        }");
}

#[test]
fn packed_byte_windows_update_the_existing_backing_storage() {
    assert_seven("machine fill(values: &write [u8; 4]) { values[1..3] = [0.1 * 70, 7u8 / 2 * 2]; }
        machine main() -> i32 {
            let mut values: [u8; 4] = \"abcd\";
            fill(&write values);
            transition values[0] == 97 && values[1] == 7 && values[2] == 6 && values[3] == 100 { true -> 7 false -> 0 }
        }");
}

#[test]
fn window_execution_rejects_stale_bounds_instead_of_clamping() {
    use typed_trees::expression::ExpressionNode;
    let program = checked(
        "machine fill(values: &write [i32; 4]) { values[1..3] = [7, 8]; }
        machine main() -> i32 { let mut values: [i32; 4]; fill(&write values); 7 }",
    );
    let end = program
        .expression_table
        .expression_entries()
        .find_map(|(_, node)| match node {
            ExpressionNode::Range(range) => Some(range.end),
            _ => None,
        })
        .expect("window end");
    for (value, message) in [
        (0, "out of bounds"),
        (2, "different element count"),
        (5, "out of bounds"),
    ] {
        let mut changed = program.clone();
        *changed.typed.expression_table.expression_mut(end) =
            ExpressionNode::Integer(numerics::literals::IntegerLiteral::from_value(value));
        let outcome = interpret_entry(&changed, "main", &[]);
        assert!(
            outcome
                .error
                .as_ref()
                .is_some_and(|error| error.contains(message)),
            "{value}: {outcome:?}"
        );
    }
}

#[test]
fn empty_windows_leave_existing_values_untouched() {
    assert_seven("machine fill(values: &write [i32; 2]) { values[1..1] = []; }
        machine main() -> i32 { let mut values: [i32; 2] = [7, 8]; fill(&write values); values[0] }");
}

#[test]
fn exact_window_elements_preserve_source_fuel_cost() {
    let source = |expression: &str| {
        format!(
            "machine fill(values: &write [i32; 1]) {{ values[0..1] = [{expression}]; }}
        machine main() -> i32 {{ let mut values: [i32; 1]; fill(&write values); values[0] }}"
        )
    };
    let anonymous = execute(&source("7 / 2 * 2"));
    let typed = execute(&source("7i32 / 2 * 2"));
    assert_eq!(anonymous.error, None);
    assert_eq!(typed.error, None);
    assert_eq!(anonymous.exit_code, 7);
    assert_eq!(typed.exit_code, 6);
    assert_eq!(anonymous.usage.fuel_units(), typed.usage.fuel_units());
}

#[test]
fn omitted_and_dynamic_windows_keep_exact_values_at_runtime() {
    for (parameters, requirements, selection, arguments) in [
        ("", "", "1..", ""),
        (
            ", start: u64, end: u64",
            "requires start <= end && end <= 4;",
            "start..end",
            ", 1, 4",
        ),
        (
            ", start: u64, end: u64",
            "requires start <= end && end < 4;",
            "start..=end",
            ", 1, 3",
        ),
    ] {
        for expression in [
            "0.1 * 70",
            "18446744073709551616 / 3 * 3 - 18446744073709551609",
        ] {
            assert_seven(&format!(
                "machine fill(values: &mut [i32; 4]{parameters}) {requirements} {{
                    values[{selection}] = [{expression}, 7i32 / 2 * 2, 22];
                }}
                machine main() -> i32 {{
                    let mut values: [i32; 4] = [11, 0, 0, 0];
                    fill(&mut values{arguments});
                    transition values[0] == 11 && values[1] == 7 && values[2] == 6 && values[3] == 22 {{ true -> 7 false -> 0 }}
                }}"
            ));
        }
    }
}

#[test]
fn slice_and_generic_windows_keep_the_integer_element_destination() {
    for (generics, collection, argument) in [
        ("", "[i32]", "&mut values"),
        ("<const N: u64>", "[i32; N]", "&mut values"),
    ] {
        assert_seven(&format!(
            "machine fill{generics}(values: &mut {collection}) {{
                values[..] = [18446744073709551616 / 3 * 3 - 18446744073709551609, 7i32 / 2 * 2];
            }}
            machine main() -> i32 {{
                let mut values: [i32; 2] = [0, 0];
                fill({argument});
                transition values[0] == 7 && values[1] == 6 {{ true -> 7 false -> 0 }}
            }}"
        ));
    }
}

#[test]
fn dynamic_window_execution_still_rejects_a_mismatched_count() {
    let outcome = execute(
        "machine fill(values: &mut [i32; 4], start: u64, end: u64)
         requires start <= end && end <= 4;
         { values[start..end] = [0.1 * 70, 8]; }
         machine main() -> i32 {
             let mut values: [i32; 4] = [0, 0, 0, 0];
             fill(&mut values, 0, 4);
             7
         }",
    );
    assert!(
        outcome
            .error
            .as_ref()
            .is_some_and(|error| error.contains("different element count")),
        "{outcome:?}"
    );
}
