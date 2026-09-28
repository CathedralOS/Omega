use super::parse_typed_trees;
use crate::CheckingRequest;
use crate::lower_typed_trees;

#[test]
fn receiver_result_bounds_do_not_survive_substitution_or_overwrite() {
    for (binding, statements, argument) in [
        ("alignment.unbounded()", "", "width"),
        ("alignment.size()", "width = 0;", "width"),
        (
            "alignment.size()",
            "let unknown: u64 = other.unbounded();",
            "unknown",
        ),
        ("alignment.size()", "", "0"),
        ("alignment.size()", "", "9"),
    ] {
        let source = receiver_bound_source(binding, statements, argument);
        let diagnostics =
            lower_typed_trees(parse_typed_trees(&source), &CheckingRequest::settled())
                .expect_err("a different or overwritten value has no getter guarantee");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("cannot prove requires contract for call consume")),
            "{diagnostics:#?}\n{source}"
        );
    }
}

fn receiver_bound_source(binding: &str, statements: &str, argument: &str) -> String {
    format!(
        "data Alignment [copy] {{ raw: u64; }}
         machine Alignment::size(&self) -> u64
         ensures result >= 1 && result <= 8 {{ 4 }}
         machine Alignment::unbounded(&self) -> u64 {{ self.raw }}
         data Probe {{ marker: u64; }}
         machine Probe::run(&self, alignment: Alignment, other: Alignment) -> u64 {{
             let mut width: u64 = {binding};
             {statements}
             transition {{ _ -> consume({argument}) }}
             state consume(&self, width: u64) -> u64
             requires width >= 1 && width <= 8 {{ width }}
         }}"
    )
}

#[test]
fn boundary_result_guarantees_bind_the_exact_invocation() {
    for (actual, after, accepted) in [
        ("input", "", true),
        ("other", "", false),
        ("input", "value.remaining = 0;", false),
        ("input", "input = 0;", false),
    ] {
        check(
            &format!(
                "data Count [copy] {{ remaining: u64; }}
                 boundary trait Counts {{
                     machine produce(input: u64) -> Count
                     ensures result.remaining == input;
                 }}
                 machine consume(value: Count, expected: u64)
                 requires value.remaining == expected {{}}
                 machine caller(service: &Counts, mut input: u64, other: u64)
                 reaches Counts
                 {{ let mut value: Count = service.produce({actual});
                    {after}
                    consume(value, input); }}"
            ),
            accepted,
        );
    }
}

#[test]
fn boundary_result_guarantees_compose_with_caller_bounds() {
    for (bound, accepted) in [("48", true), ("47", false)] {
        check(
            &format!(
                "data Count [copy] {{ remaining: u64; }}
                 boundary trait Counts {{
                     machine produce(input: u64, requested: u64) -> Count
                     requires requested <= input
                     ensures result.remaining == input - requested;
                 }}
                 machine consume(value: Count) requires 32 <= value.remaining {{}}
                 machine caller(service: &Counts, input: u64)
                 requires {bound} <= input
                 reaches Counts
                 {{ let value: Count = service.produce(input, 16);
                    consume(value); }}"
            ),
            accepted,
        );
    }
}

fn check(source: &str, accepted: bool) {
    match lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled()) {
        Ok(_) => assert!(accepted, "unproved caller requirement accepted:\n{source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics.iter().any(|diagnostic| diagnostic
                    .message
                    .contains("cannot prove requires contract for call consume")),
                "expected the consumer requirement to reject: {diagnostics:#?}\n{source}"
            );
        }
    }
}

#[test]
fn caller_result_relations_preserve_nested_computations() {
    for (right, accepted) in [
        ("capacity - length", true),
        ("capacity", false),
        ("capacity - other", false),
    ] {
        check(&format!("data Count [copy] {{ remaining: u64; }}
            data Issued [copy] {{ count: Count; }}
            machine produce(capacity: u64, length: u64) -> Issued
            requires length <= capacity
            ensures result.count.remaining == capacity - length
            {{ Issued {{ count: Count {{ remaining: capacity - length }} }} }}
            machine consume(value: Issued, capacity: u64, length: u64, other: u64)
            requires length <= capacity; other <= capacity; value.count.remaining == {right}
            {{}}
            machine caller(capacity: u64, length: u64, other: u64)
            requires length <= capacity; other <= capacity
            {{ let value: Issued = produce(capacity, length); consume(value, capacity, length, other); }}"), accepted);
    }
}

#[test]
fn caller_result_relations_keep_arithmetic_policy() {
    check(
        "data Count [copy] { remaining: u8; }
        machine produce(input: u8) -> Count
        ensures result.remaining == ((input as u8 in Wrapping) + 1) as u8
        { Count { remaining: ((input as u8 in Wrapping) + 1) as u8 } }
        machine consume(value: Count, expected: u8)
        requires value.remaining == ((expected as u8 in Saturating) + 1) as u8 {}
        machine caller(input: u8) { let value: Count = produce(input); consume(value, input); }",
        false,
    );
}

#[test]
fn caller_result_relations_require_capture_preservation_not_purity() {
    for (argument, accepted) in [("input", true), ("current", false)] {
        check(
            &format!(
                "data Count [copy] {{ remaining: u64; }}
            machine produce(input: u64, target: &mut u64) -> Count
            ensures result.remaining == input
            {{ target = 8; Count {{ remaining: input }} }}
            machine consume(value: Count, expected: u64)
            requires value.remaining == expected {{}}
            machine caller(input: u64) {{
                let mut current: u64 = input;
                let value: Count = produce({argument}, &mut current);
                consume(value, {argument});
            }}"
            ),
            accepted,
        );
    }
}
