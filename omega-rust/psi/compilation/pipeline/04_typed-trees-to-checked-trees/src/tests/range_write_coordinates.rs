use crate::borrow::build_borrow_facts;
use crate::tests::front_end::{checked_program_result, typed_program};

fn check(source: &str, accepted: bool) {
    match checked_program_result(source) {
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
            let program = typed_program(&source);
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
                &crate::flow::StateMutationSummaryCache::default(),
                ::validation::CallFrameResolver::new(&program).as_ref(),
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
