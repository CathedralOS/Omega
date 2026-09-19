use super::parse_typed_trees;
use crate::lower_typed_trees;

fn check(source: &str, accepted: bool) {
    match lower_typed_trees(parse_typed_trees(source)) {
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
fn result_field_guarantee_is_available_to_a_caller() {
    check(
        "data Count [copy] { remaining: u64; }
         machine produce(input: u64) -> Count
         ensures result.remaining == input
         { Count { remaining: input } }
         machine consume(value: Count, expected: u64)
         requires value.remaining == expected
         {}
         machine caller(input: u64) {
             let value: Count = produce(input);
             consume(value, input);
         }",
        true,
    );
}

#[test]
fn caller_scalar_bound_survives_unrelated_owned_transfer() {
    for (parameters, arguments) in [
        ("capacity: u64", "capacity"),
        ("unused: u64, capacity: u64", "moved.length, capacity"),
    ] {
        check(
            &format!(
                "data Backing {{ length: u64; }}
                 machine transfer(backing: Backing) -> Backing {{ backing }}
                 machine consume({parameters}) requires 16 <= capacity {{}}
                 machine caller(backing: Backing, capacity: u64)
                 requires 48 <= capacity; capacity == backing.length
                 {{
                     let moved: Backing = transfer(backing);
                     consume({arguments});
                 }}"
            ),
            true,
        );
    }
}

#[test]
fn unused_result_fields_do_not_supply_missing_scalar_bounds() {
    for (bound, actual, prefix) in [
        ("8", "capacity", ""),
        ("48", "other", ""),
        ("48", "capacity", "capacity = 0;"),
    ] {
        check(
            &format!(
                "data Backing {{ length: u64; }}
                 machine transfer(backing: Backing) -> Backing {{ backing }}
                 machine consume(unused: u64, capacity: u64) requires 16 <= capacity {{}}
                 machine caller(backing: Backing, mut capacity: u64, other: u64)
                 requires {bound} <= capacity
                 {{
                     let moved: Backing = transfer(backing);
                     {prefix}
                     consume(moved.length, {actual});
                 }}"
            ),
            false,
        );
    }
}

#[test]
fn caller_result_relations_follow_live_copy_provenance() {
    for (body, accepted) in [
        (
            "let capacity: u64 = 48; let value: Count = produce(capacity); consume(value, capacity);",
            true,
        ),
        (
            "let value: Count = produce(input); consume(value, input);",
            true,
        ),
        (
            "let value: Count = produce(input); let copied: Count = value; consume(copied, input);",
            true,
        ),
        (
            "let mut value: Count = produce(input); let copied: Count = value; value.remaining = 8; consume(copied, input);",
            true,
        ),
        (
            "let mut value: Count = produce(input); value.remaining = 8; consume(value, input);",
            false,
        ),
        (
            "let mut value: Count = produce(input); replace(&mut value.remaining); consume(value, input);",
            false,
        ),
        (
            "let value: Count = produce(input); consume(value, other);",
            false,
        ),
        (
            "let value: Count = produce(input); let other_value: Count = produce(other); consume(other_value, input);",
            false,
        ),
        (
            "let value: Count = produce(input); let mut copied: Count = value; copied.remaining = 8; consume(copied, input);",
            false,
        ),
        (
            "let mut current: u64 = input; let value: Count = produce(current); current = other; consume(value, current);",
            false,
        ),
    ] {
        check(&format!("data Count [copy] {{ remaining: u64; }}
            machine produce(input: u64) -> Count ensures result.remaining == input {{ Count {{ remaining: input }} }}
            machine replace(value: &mut u64) {{ value = 8; }}
            machine consume(value: Count, expected: u64) requires value.remaining == expected {{}}
            machine caller(input: u64, other: u64) {{ {body} }}"), accepted);
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
fn caller_result_relations_do_not_confuse_computed_argument_snapshots() {
    check("data Count [copy] { remaining: u64; }
        machine produce(input: u64) -> Count ensures result.remaining == input { Count { remaining: input } }
        machine consume(value: Count, expected: u64) requires value.remaining == expected {}
        machine caller(mut input: u64) {
            let value: Count = produce((input as u64 in Wrapping) + 1);
            input = 0;
            consume(value, (input as u64 in Wrapping) + 1);
        }", false);
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
fn caller_result_relations_project_constructor_inputs_and_invalidate_dependencies() {
    for (after, accepted) in [("", true), ("capacity = 8;", false)] {
        check(
            &format!(
                "data Count [copy] {{ remaining: u64; }}
            data Issued [copy] {{ count: Count; }}
            machine produce(source: Count) -> Issued
            ensures result.count.remaining == source.remaining
            {{ Issued {{ count: Count {{ remaining: source.remaining }} }} }}
            machine consume(value: &Issued, expected: u64)
            requires value.count.remaining == expected {{}}
            machine caller(mut capacity: u64) {{
                let value: Issued = produce(Count {{ remaining: capacity }});
                {after}
                consume(&value, capacity);
            }}"
            ),
            accepted,
        );
    }
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
