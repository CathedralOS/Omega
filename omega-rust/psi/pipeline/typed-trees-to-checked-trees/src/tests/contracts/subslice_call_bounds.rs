use super::{lower_typed_trees, parse_typed_trees};

fn window_source(callees: &str, selection: &str) -> String {
    format!(
        r#"
        {callees}
        machine window(items: &[i32; 4]) -> u64 {{
            let view: &[i32] = items[{selection}];
            view.len
        }}
        "#,
    )
}

fn check_window(source: &str, accepted: bool) {
    // Parsing, resolution, and typing must succeed before a bounds refusal counts.
    let typed = parse_typed_trees(source);
    match lower_typed_trees(typed) {
        Ok(_) => assert!(accepted, "unproven call-bounded window accepted:\n{source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics
                    .iter()
                    .all(|diagnostic| diagnostic.message.contains("cannot prove")
                        && diagnostic.message.contains("subslice range")),
                "expected only subslice bounds diagnostics: {diagnostics:#?}\n{source}"
            );
            assert!(
                !diagnostics.is_empty(),
                "missing bounds diagnostic:\n{source}"
            );
        }
    }
}

#[test]
fn exact_call_return_ranges_prove_start_end_and_omitted_endpoints() {
    for selection in [
        "endpoint()..4",
        "endpoint()..",
        "0..endpoint()",
        "..endpoint()",
        "endpoint()..=3",
        "0..=endpoint()",
        "..=endpoint()",
    ] {
        check_window(
            &window_source("machine endpoint() -> u64 [0..=3] { 2 }", selection),
            true,
        );
    }
}

#[test]
fn signed_nonnegative_call_return_ranges_prove_both_endpoints() {
    for selection in [
        "endpoint()..4",
        "endpoint()..",
        "..endpoint()",
        "..=endpoint()",
    ] {
        check_window(
            &window_source("machine endpoint() -> i64 [0..=3] { 2 }", selection),
            true,
        );
    }
}

#[test]
fn exclusive_call_endpoint_may_equal_the_array_length() {
    for selection in ["..endpoint()", "endpoint()..", "endpoint()..4"] {
        check_window(
            &window_source("machine endpoint() -> u64 [4..=4] { 4 }", selection),
            true,
        );
    }
}

#[test]
fn distinct_call_intervals_establish_ordering_including_touching_bounds() {
    for (start_type, end_type, selection) in [
        ("u64 [0..=1]", "u64 [2..=3]", "first()..last()"),
        ("u64 [0..=2]", "u64 [2..=3]", "first()..last()"),
        ("i64 [0..=1]", "i64 [2..=3]", "first()..=last()"),
        // Inclusive normalization permits start == end + 1, an empty window.
        ("u64 [0..=2]", "u64 [1..=3]", "first()..=last()"),
    ] {
        let callees = format!(
            "machine first() -> {start_type} {{ 1 }}
             machine last() -> {end_type} {{ 2 }}"
        );
        check_window(&window_source(&callees, selection), true);
    }
}

#[test]
fn literal_start_precedes_the_call_return_range_minimum() {
    for selection in ["1..endpoint()", "2..endpoint()", "3..=endpoint()"] {
        check_window(
            &window_source("machine endpoint() -> u64 [2..=3] { 2 }", selection),
            true,
        );
    }
}

#[test]
fn possibly_negative_call_return_ranges_cannot_prove_start_or_end() {
    for selection in [
        "endpoint()..4",
        "endpoint()..",
        "..endpoint()",
        "..=endpoint()",
    ] {
        check_window(
            &window_source("machine endpoint() -> i64 [-1..=2] { -1 }", selection),
            false,
        );
    }
}

#[test]
fn reversed_and_overlapping_call_intervals_leave_ordering_unproven() {
    for (start_type, start_value, end_type, end_value, selection) in [
        ("u64 [3..=3]", "3", "u64 [1..=1]", "1", "first()..last()"),
        ("u64 [0..=3]", "3", "u64 [1..=4]", "1", "first()..last()"),
        ("u64 [0..=3]", "3", "u64 [1..=3]", "1", "first()..=last()"),
    ] {
        let callees = format!(
            "machine first() -> {start_type} {{ {start_value} }}
             machine last() -> {end_type} {{ {end_value} }}"
        );
        check_window(&window_source(&callees, selection), false);
    }
}

#[test]
fn two_identically_spelled_calls_do_not_establish_endpoint_ordering() {
    // The first call can return 3 and the second 0 despite identical spelling.
    let source = r#"
        machine endpoint(cursor: &mut u64 [0..=4]) -> u64 [0..=4] {
            let previous: u64 [0..=4] = cursor;
            cursor = 0;
            previous
        }
        machine window(items: &[i32; 4]) -> u64 {
            let mut cursor: u64 [0..=4] = 3;
            let view: &[i32] = items[endpoint(&mut cursor)..endpoint(&mut cursor)];
            view.len
        }
    "#;
    check_window(source, false);
}

#[test]
fn call_return_range_cannot_extend_beyond_the_array() {
    for selection in ["..endpoint()", "endpoint()..", "endpoint()..4"] {
        check_window(
            &window_source("machine endpoint() -> u64 [0..=5] { 5 }", selection),
            false,
        );
    }
}

#[test]
fn inclusive_call_endpoint_at_array_length_is_rejected() {
    check_window(
        &window_source("machine endpoint() -> u64 [4..=4] { 4 }", "..=endpoint()"),
        false,
    );
}

#[test]
fn an_unranged_call_return_does_not_prove_an_endpoint_from_its_body() {
    for selection in ["endpoint()..4", "..endpoint()"] {
        check_window(
            &window_source("machine endpoint() -> u64 { 2 }", selection),
            false,
        );
    }
}

#[test]
fn permissive_return_range_declarations_reject_before_bounds() {
    for policy in ["Wrapping", "Saturating"] {
        for selection in ["endpoint()..4", "..endpoint()", "..=endpoint()"] {
            let callees = format!("machine endpoint() -> u64 [0..=3] in {policy} {{ 2 }}");
            let source = window_source(&callees, selection);
            let diagnostics = lower_typed_trees(parse_typed_trees(&source))
                .expect_err("a permissive policy cannot promise an enforced return range");
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic
                    .message
                    .contains("return type declares a range constraint")
                    && diagnostic.message.contains(policy)),
                "{diagnostics:#?}"
            );
        }
    }
}

#[test]
fn a_callee_must_independently_establish_its_declared_return_range() {
    for returned in ["-1", "5"] {
        let source = format!("machine endpoint() -> i64 [0..=3] {{ {returned} }}");
        let diagnostics = lower_typed_trees(parse_typed_trees(&source))
            .expect_err("a false return range must reject even without a subslice caller");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("returns a value not provably within its declared range")),
            "expected an independent return proof refusal: {diagnostics:#?}\n{source}"
        );
    }
}

#[test]
fn a_literal_call_return_range_does_not_establish_runtime_slice_length() {
    for selection in ["endpoint()..", "..endpoint()", "..=endpoint()"] {
        let source = format!(
            r#"
            machine endpoint() -> u64 [1..=1] {{ 1 }}
            machine window(items: &[i32]) -> u64 {{
                let view: &[i32] = items[{selection}];
                view.len
            }}
            "#,
        );
        check_window(&source, false);
    }
}

#[test]
fn ordinary_method_calls_use_the_selected_receivers_return_range() {
    for (receiver_type, accepted) in [("Narrow", true), ("Wide", false)] {
        let source = format!(
            r#"
            data Narrow {{}}
            data Wide {{}}
            machine Narrow::endpoint(&self) -> u64 [0..=3] {{ 2 }}
            machine Wide::endpoint(&self) -> u64 [0..=5] {{ 5 }}
            machine window(items: &[i32; 4], receiver: &{receiver_type}) -> u64 {{
                let view: &[i32] = items[..receiver.endpoint()];
                view.len
            }}
            "#,
        );
        check_window(&source, accepted);
    }
}
