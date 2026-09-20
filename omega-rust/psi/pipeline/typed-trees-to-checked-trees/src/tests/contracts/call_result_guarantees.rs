use super::parse_typed_trees;
use crate::lower_typed_trees;

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
fn independent_postconditions_keep_separate_storage_dependencies() {
    for (update, accepted) in [("observed = 0;", true), ("input = 0;", false)] {
        check(
            &format!(
                "data Counts [copy] {{ primary: u64; secondary: u64; }}
             boundary trait Values {{ machine read(input: u64, observed: &u64) -> Counts
                 ensures result.primary == input, result.secondary == observed; }}
             machine consume(value: u64, expected: u64) requires value == expected {{}}
             machine caller(service: &Values, mut input: u64) reaches Values {{
                 let mut observed: u64 = 17;
                 let value: Counts = service.read(input, &observed);
                 {update}
                 consume(value.primary, input);
             }}"
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

#[test]
fn boundary_result_guarantees_preserve_input_write_ceilings() {
    for (argument, accepted) in [("input", true), ("current", false)] {
        check(
            &format!(
                "data Count [copy] {{ remaining: u64; }}
                 boundary trait Counts {{
                     machine produce(input: u64, target: &mut u64) -> Count
                     ensures result.remaining == input;
                 }}
                 machine consume(value: Count, expected: u64)
                 requires value.remaining == expected {{}}
                 machine caller(service: &Counts, input: u64) reaches Counts {{
                     let mut current: u64 = input;
                     let value: Count = service.produce({argument}, &mut current);
                     consume(value, {argument});
                 }}"
            ),
            accepted,
        );
    }
}

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
fn constructed_field_bounds_use_caller_facts_without_a_return_guarantee() {
    for (remaining, after, accepted) in [
        ("capacity", "", true),
        ("other", "", false),
        ("capacity", "capacity = 0;", false),
    ] {
        check(
            &format!(
                "data Token {{ value: u64; }}
                 data Count [copy] {{ remaining: u64; }}
                 data Request {{ token: Token; count: Count; }}
                 machine make() -> Token {{ Token {{ value: 0 }} }}
                 machine consume(value: Request, needed: u64) -> Token
                 requires needed <= value.count.remaining
                 {{ value.token }}
                 machine caller(mut capacity: u64, needed: u64, other: u64) -> Token
                 requires needed <= capacity
                 {{
                     let token: Token = make();
                     {after}
                     consume(Request {{ token: token, count: Count {{ remaining: {remaining} }} }}, needed)
                 }}"
            ),
            accepted,
        );
    }
}

#[test]
fn returned_residual_establishes_the_next_capacity_requirement() {
    for (bound, source, after, accepted) in [
        ("48", "capacity", "", true),
        ("47", "capacity", "", false),
        ("48", "other", "", false),
        ("48", "capacity", "value.count.remaining = 0;", false),
        ("48", "capacity", "capacity = 16;", false),
    ] {
        check(
            &format!(
                "data Count [copy] {{ remaining: u64; }}
             data Issued [copy] {{ count: Count; }}
             machine produce(capacity: u64, length: u64) -> Issued
             requires length <= capacity
             ensures result.count.remaining == capacity - length
             {{ Issued {{ count: Count {{ remaining: capacity - length }} }} }}
             machine consume(value: Issued) requires 32 <= value.count.remaining {{}}
             machine caller(mut capacity: u64, other: u64)
             requires {bound} <= capacity; 16 <= other
             {{
                 let mut value: Issued = produce({source}, 16);
                 {after}
                 consume(value);
             }}"
            ),
            accepted,
        );
    }
}

#[test]
fn residual_bounds_follow_live_copies_and_distinct_invocations() {
    for (body, accepted) in [
        (
            "let copied: Issued = value; value.count.remaining = 0; consume(copied);",
            true,
        ),
        (
            "let mut copied: Issued = value; copied.count.remaining = 0; consume(copied);",
            false,
        ),
        ("value = produce(other, 16); consume(value);", false),
        (
            "let later: Issued = produce(other, 16); consume(later);",
            false,
        ),
    ] {
        check(
            &format!(
                "data Count [copy] {{ remaining: u64; }}
             data Issued [copy] {{ count: Count; }}
             machine produce(capacity: u64, length: u64) -> Issued
             requires length <= capacity
             ensures result.count.remaining == capacity - length
             {{ Issued {{ count: Count {{ remaining: capacity - length }} }} }}
             machine consume(value: Issued) requires 32 <= value.count.remaining {{}}
             machine caller(capacity: u64, other: u64)
             requires 48 <= capacity; 16 <= other
             {{ let mut value: Issued = produce(capacity, 16); {body} }}"
            ),
            accepted,
        );
    }
}

#[test]
fn residual_bounds_do_not_reinterpret_wrapping_arithmetic() {
    check(
        "data Count [copy] { remaining: u8; }
         machine produce(input: u8) -> Count
         ensures result.remaining == ((input as u8 in Wrapping) + 1) as u8
         { Count { remaining: ((input as u8 in Wrapping) + 1) as u8 } }
         machine consume(value: Count) requires 1 <= value.remaining {}
         machine caller(input: u8) requires input == 255 {
             let value: Count = produce(input);
             consume(value);
         }",
        false,
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
#[test]
fn embedded_call_guarantees_preserve_sibling_operand_order() {
    for (first, second, accepted) in [
        ("replace(&mut current)", "expect(&current)", false),
        ("expect(&current)", "replace(&mut current)", true),
    ] {
        let source = format!(
            "data Count [copy] {{ remaining: u64; }}
             data Pair [copy] {{ first: u64; second: u64; }}
             machine make() -> Count ensures embed(result.remaining) == 7
             {{ Count {{ remaining: 7 }} }}
             machine replace(value: &mut Count) -> u64
             {{ value = Count {{ remaining: 8 }}; 0 }}
             machine expect(value: &Count) -> u64
             requires embed(value.remaining) == 7 {{ 0 }}
             machine use_result() {{ let mut current: Count = make();
                 let pair: Pair = Pair {{ first: {first}, second: {second} }}; }}"
        );
        let result = crate::lower_typed_trees(super::parse_typed_trees(&source));
        assert_eq!(result.is_ok(), accepted, "{result:#?}\n{source}");
    }
}
