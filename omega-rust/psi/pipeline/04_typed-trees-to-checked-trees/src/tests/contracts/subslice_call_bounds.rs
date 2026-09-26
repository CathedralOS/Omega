use super::super::lower_typed_trees;
use super::parse_typed_trees;
use crate::CheckingRequest;

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
    match lower_typed_trees(typed, &CheckingRequest::settled()) {
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
fn call_return_range_cannot_extend_beyond_the_array() {
    for selection in ["..endpoint()", "endpoint()..", "endpoint()..4"] {
        check_window(
            &window_source("machine endpoint() -> u64 [0..=5] { 5 }", selection),
            false,
        );
    }
}
