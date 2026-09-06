use super::*;

fn check(source: &str, accepted: bool) {
    let tokens = Lexer::new(source)
        .tokenize()
        .unwrap_or_else(|diagnostics| panic!("tokenize: {diagnostics:#?}\n{source}"));
    let syntax = parse_syntax_trees(&tokens)
        .unwrap_or_else(|diagnostics| panic!("parse: {diagnostics:#?}\n{source}"));
    let resolved = lower_syntax_trees(&syntax)
        .unwrap_or_else(|diagnostics| panic!("resolve: {diagnostics:#?}\n{source}"));
    let typed = lower_symbol_resolved_trees(&resolved)
        .unwrap_or_else(|diagnostics| panic!("type: {diagnostics:#?}\n{source}"));
    match lower_typed_trees(typed) {
        Ok(_) => assert!(accepted, "stale projected bound accepted: {source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic.is_error()),
                "expected an error: {diagnostics:#?}\n{source}"
            );
            assert!(
                diagnostics
                    .iter()
                    .filter(|diagnostic| diagnostic.is_error())
                    .all(|diagnostic| diagnostic.message.contains("cannot prove subslice range")),
                "expected only subslice range errors, not earlier proof or authority failures: \
                 {diagnostics:#?}\n{source}"
            );
        }
    }
}

fn source(declarations: &str, parameters: &str, boundary: &str, body: &str) -> String {
    format!(
        "{declarations}
         machine set(output: &mut i64, replacement: i64) {{ output = replacement; }}
         machine window(items: &[i32; 4], {parameters}, replacement: i64) -> u64
         requires 0 <= {boundary} && {boundary} <= 4; {{
             {body}
             let view: &[i32] = items[0..{boundary}];
             view.len
         }}"
    )
}

fn element_source(body: &str) -> String {
    source(
        "machine inspect(value: &i64) {}",
        "original: &mut [i64; 2], index: u64 [0..=1]",
        "original[0]",
        body,
    )
}

#[test]
fn element_aliases_and_projected_calls_preserve_only_disjoint_bounds() {
    for (body, accepted) in [
        ("", true),
        ("inspect(&original[0]);", true),
        ("original[1] = replacement;", true),
        ("original[0] = replacement;", false),
        ("set(&mut original[1], replacement);", true),
        ("set(&mut original[0], replacement);", false),
        (
            "let alias: &mut i64 = &mut original[1]; alias = replacement;",
            true,
        ),
        (
            "let alias: &mut i64 = &mut original[0]; alias = replacement;",
            false,
        ),
        (
            "let alias: &mut i64 = &mut original[1]; set(alias, replacement);",
            true,
        ),
        (
            "let alias: &mut i64 = &mut original[0]; set(alias, replacement);",
            false,
        ),
    ] {
        check(&element_source(body), accepted);
    }
}

#[test]
fn dynamic_element_aliases_and_calls_cannot_preserve_a_fixed_bound() {
    // Declared selector bounds keep every indexed access legal independently
    // of the premise that the write must invalidate.
    for body in [
        "original[index] = replacement;",
        "set(&mut original[index], replacement);",
        "let alias: &mut i64 = &mut original[index]; alias = replacement;",
        "let alias: &mut i64 = &mut original[index]; set(alias, replacement);",
    ] {
        check(&element_source(body), false);
    }
}

#[test]
fn fields_below_elements_keep_both_coordinates_through_aliases_and_calls() {
    for (body, accepted) in [
        ("", true),
        ("set(&mut cells[0].other, replacement);", true),
        ("set(&mut cells[1].end, replacement);", true),
        ("set(&mut cells[0].end, replacement);", false),
        (
            "let alias: &mut i64 = &mut cells[0].other; alias = replacement;",
            true,
        ),
        (
            "let alias: &mut i64 = &mut cells[0].end; alias = replacement;",
            false,
        ),
        (
            "let alias: &mut i64 = &mut cells[0].other; set(alias, replacement);",
            true,
        ),
        (
            "let alias: &mut i64 = &mut cells[0].end; set(alias, replacement);",
            false,
        ),
        (
            "let alias: &mut Endpoint = &mut cells[0]; alias.other = replacement;",
            true,
        ),
        (
            "let alias: &mut Endpoint = &mut cells[0]; alias.end = replacement;",
            false,
        ),
        (
            "let alias: &mut Endpoint = &mut cells[0]; set_other(alias, replacement);",
            true,
        ),
        (
            "let alias: &mut Endpoint = &mut cells[0]; set_end(alias, replacement);",
            false,
        ),
        ("set_end(&mut cells[1], replacement);", true),
        ("set_end(&mut cells[index], replacement);", false),
    ] {
        check(
            &source(
                "data Endpoint { end: i64; other: i64; }
                 machine set_other(output: &mut Endpoint, replacement: i64) {
                     output.other = replacement;
                 }
                 machine set_end(output: &mut Endpoint, replacement: i64) {
                     output.end = replacement;
                 }",
                "cells: &mut [Endpoint; 2], index: u64 [0..=1]",
                "cells[0].end",
                body,
            ),
            accepted,
        );
    }
}

#[test]
fn nested_arrays_keep_each_fixed_coordinate_and_overlapping_prefix() {
    for (body, accepted) in [
        ("", true),
        ("set(&mut rows[0][1], replacement);", true),
        ("set(&mut rows[1][0], replacement);", true),
        ("set(&mut rows[0][0], replacement);", false),
        (
            "let alias: &mut i64 = &mut rows[0][1]; alias = replacement;",
            true,
        ),
        (
            "let alias: &mut i64 = &mut rows[0][0]; alias = replacement;",
            false,
        ),
        (
            "let row: &mut [i64; 2] = &mut rows[0]; set(&mut row[1], replacement);",
            true,
        ),
        (
            "let row: &mut [i64; 2] = &mut rows[0]; set(&mut row[0], replacement);",
            false,
        ),
        ("replace_row(&mut rows[1], replacement);", true),
        ("replace_row(&mut rows[0], replacement);", false),
        ("set(&mut rows[index][0], replacement);", false),
        ("set(&mut rows[0][index], replacement);", false),
    ] {
        check(
            &source(
                "machine replace_row(output: &mut [i64; 2], replacement: i64) {
                     output = [replacement, replacement];
                 }",
                "rows: &mut [[i64; 2]; 2], index: u64 [0..=1]",
                "rows[0][0]",
                body,
            ),
            accepted,
        );
    }
}

#[test]
fn whole_replacements_invalidate_descendant_bounds_through_aliases_and_calls() {
    for body in [
        "original = [replacement, replacement];",
        "replace(original, replacement);",
        "let alias: &mut [i64; 2] = original; alias = [replacement, replacement];",
        "let alias: &mut [i64; 2] = original; replace(alias, replacement);",
    ] {
        check(
            &source(
                "machine replace(output: &mut [i64; 2], replacement: i64) {
                     output = [replacement, replacement];
                 }",
                "original: &mut [i64; 2]",
                "original[0]",
                body,
            ),
            false,
        );
    }
}

#[test]
fn rebinding_an_element_alias_does_not_retarget_an_earlier_reference_copy() {
    for (initial, rebound) in [(0, 1), (1, 0)] {
        for (mutation, accepted) in [
            ("", true),
            ("prior = replacement;", initial != 0),
            ("set(prior, replacement);", initial != 0),
            ("alias = replacement;", rebound != 0),
            ("set(alias, replacement);", rebound != 0),
        ] {
            check(
                &element_source(&format!(
                    "let mut alias: &mut i64 = &mut original[{initial}];
                     let prior: &mut i64 = alias;
                     alias = &mut original[{rebound}];
                     {mutation}"
                )),
                accepted,
            );
        }
    }
}

#[test]
fn projected_reborrows_keep_their_source_when_the_parent_binding_changes() {
    // The child captures a field of the old element, not a route through the
    // parent binding that can be reevaluated after the replacement.
    for (initial, rebound) in [(0, 1), (1, 0)] {
        for (mutation, accepted) in [
            ("", true),
            ("prior = replacement;", initial != 0),
            ("set(prior, replacement);", initial != 0),
        ] {
            check(
                &source(
                    "data Endpoint { end: i64; other: i64; }",
                    "cells: &mut [Endpoint; 2]",
                    "cells[0].end",
                    &format!(
                        "let mut alias: &mut Endpoint = &mut cells[{initial}];
                         let prior: &mut i64 = &mut alias.end;
                         alias = &mut cells[{rebound}];
                         {mutation}"
                    ),
                ),
                accepted,
            );
        }
    }
}

#[test]
fn selector_mutation_cannot_retarget_an_existing_element_reference() {
    for mutation in ["alias = replacement;", "set(alias, replacement);"] {
        check(
            &source(
                "",
                "original: &mut [i64; 2], mut index: u64 [0..=1]",
                "original[0]",
                &format!("let alias: &mut i64 = &mut original[index]; index = 1; {mutation}"),
            ),
            false,
        );
    }
}

#[test]
fn expression_calls_and_nested_call_summaries_keep_exact_fixed_writes() {
    for (body, accepted) in [
        (
            "let ignored: i64 = write_value(&mut original[1], replacement);",
            true,
        ),
        (
            "let ignored: i64 = write_value(&mut original[0], replacement);",
            false,
        ),
        ("write_second(original, replacement);", true),
        (
            "set(&mut original[1], write_value(&mut original[0], replacement));",
            false,
        ),
    ] {
        check(
            &source(
                "machine write_value(output: &mut i64, replacement: i64) -> i64 { output = replacement; 0 }
                 machine write_second(output: &mut [i64; 2], replacement: i64) { set(&mut output[1], replacement); }",
                "original: &mut [i64; 2]",
                "original[0]",
                body,
            ),
            accepted,
        );
    }
}

#[test]
fn transitive_write_selectors_require_builtin_arithmetic_meaning() {
    for (declaration, precise) in [
        ("", true),
        (
            "operator + u64::custom(left: u64, right: u64) -> u64;",
            false,
        ),
        (
            "operator + f64::unrelated(left: f64, right: f64) -> f64;",
            true,
        ),
    ] {
        for call in [
            "set(&mut original[0u64 + 1u64], replacement);",
            "relay(original, replacement);",
        ] {
            let source = format!(
                "{declaration}
                 machine set(output: &mut i64, replacement: i64) {{ output = replacement; }}
                 machine relay(output: &mut [i64; 2], replacement: i64) {{
                     set(&mut output[0u64 + 1u64], replacement);
                 }}
                 machine caller(original: &mut [i64; 2], replacement: i64) {{ {call} }}"
            );
            let tokens = Lexer::new(&source).tokenize().expect("tokens");
            let syntax = parse_syntax_trees(&tokens).expect("syntax");
            let resolved = lower_syntax_trees(&syntax).expect("resolved");
            let program = lower_symbol_resolved_trees(&resolved).expect("typed");
            let machine = program
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "caller")
                .expect("caller");
            let state = &program.machine_states(machine)[0];
            let borrows = build_borrow_facts(&program);
            let row = borrows
                .states
                .iter()
                .map(|(_, row)| row)
                .find(|row| row.state_symbol == state.symbol)
                .expect("caller borrows");
            let [call] = borrows.calls.span_or_empty(row.calls) else {
                panic!("one caller call");
            };
            let writes = crate::flow::call_mutated_places(
                &program,
                machine.symbol,
                state.symbol,
                &borrows,
                call,
                &mut crate::flow::StateMutationSummaryCache::default(),
            );
            if precise {
                let writes = writes.expect("complete builtin writes");
                assert!(
                    writes
                        .iter()
                        .any(|place| place.segments
                            == [facts::PlaceSegment::FixedIndex { index: 1 }]),
                    "{source}\n{writes:?}"
                );
            } else if let Some(writes) = writes {
                assert!(
                    !writes.is_empty(),
                    "custom selector cannot erase writes: {source}"
                );
                assert!(
                    writes.iter().all(|place| !place
                        .segments
                        .iter()
                        .any(|segment| matches!(segment, facts::PlaceSegment::FixedIndex { .. }))),
                    "custom selector cannot invent fixed coordinates: {source}\n{writes:?}"
                );
            }
        }
    }
}
