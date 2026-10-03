use super::{ArithmeticDomain, BinaryOperator, ExactDefinednessConditions, definedness_conditions};

#[test]
fn exact_division_definedness_follows_the_shared_policy_catalog() {
    assert_eq!(
        definedness_conditions(BinaryOperator::Divide, ArithmeticDomain::Exact),
        ExactDefinednessConditions {
            nonzero_divisor: true,
            signed_result_representable: true,
        }
    );
}

fn check(source: &str, accepted: bool) {
    let program = crate::front_end::typed_program(source);
    let result = crate::validate_program(&program);
    if accepted {
        assert!(result.is_ok(), "{source}\n{result:?}");
    } else {
        let errors = result
            .err()
            .expect("unproved operation must reject in checking");
        assert!(
            errors.iter().any(
                |error| error.message.contains("divisor must be proven nonzero")
                    || error.message.contains("signed `MIN / -1` primitive pair")
                    || error.message.contains("by zero")
            ),
            "{source}\n{errors:?}"
        );
    }
}

#[test]
fn runtime_division_policies_require_exact_definedness() {
    for operator in ["/", "%"] {
        for carrier in ["u32", "u32 in Wrapping", "u32 in Saturating"] {
            for (requirement, accepted) in [("", false), ("requires divisor != 0", true)] {
                check(
                    &format!(
                        "machine value(input: {carrier}, divisor: {carrier}) -> {carrier} {requirement} {{ input {operator} divisor }}"
                    ),
                    accepted,
                );
            }
        }
        // Trapping retains its own crash predicate instead of a static
        // nonzero or signed-result formation condition.
        check(
            &format!(
                "machine value(input: i32 in Trapping, divisor: i32 in Trapping) -> i32 in Trapping {{ input {operator} divisor }}"
            ),
            true,
        );
        for (requirement, accepted) in [
            ("divisor != 0", false),
            ("divisor != 0 && divisor != -1", true),
            ("divisor != 0 && input != -2147483648", true),
            ("divisor >= 1", true),
        ] {
            check(
                &format!(
                    "machine value(input: i32, divisor: i32) -> i32 requires {requirement} {{ input {operator} divisor }}"
                ),
                accepted,
            );
        }
        for policy in ["Wrapping", "Saturating"] {
            check(
                &format!(
                    "machine value(input: i32 in {policy}, divisor: i32 in {policy}) -> i32 in {policy} requires divisor != 0 {{ input {operator} divisor }}"
                ),
                true,
            );
        }
    }
}

#[test]
fn runtime_divisor_guards_rebind_and_invalidate_current_values() {
    for operator in ["/", "%"] {
        check(
            &format!(
                "machine value(divisor: i32) -> i32 {{ transition divisor != 0 {{ true -> 12i32 {operator} divisor false -> 0i32 }} }}"
            ),
            true,
        );
        check(
            &format!(
                "machine value(divisor: i32) -> i32 {{ transition divisor != 0 {{ true -> finish(divisor) false -> 0i32 }} state finish(current: i32) -> i32 {{ 12i32 {operator} current }} }}"
            ),
            true,
        );
        check(
            &format!(
                "machine value(divisor: i32, other: i32) -> i32 requires other != 0 {{ 12i32 {operator} divisor }}"
            ),
            false,
        );
        check(
            &format!(
                "data Slot {{ divisor: i32; }} machine value(slot: &mut Slot, replacement: i32) -> i32 requires slot.divisor != 0 {{ slot.divisor = replacement; 12i32 {operator} slot.divisor }}"
            ),
            false,
        );
        check(
            &format!(
                "machine value() -> i32 {{ let mut divisor: i32 = 2; divisor = 3; 12i32 {operator} divisor }}"
            ),
            true,
        );
        check(
            &format!(
                "machine value() -> i32 {{ let mut divisor: i32 = 2; divisor = 0; 12i32 {operator} divisor }}"
            ),
            false,
        );
    }
}

#[test]
fn runtime_divisor_discharge_respects_selected_operator_meaning() {
    for (operator, name) in [("/", "divide"), ("%", "remainder")] {
        check(
            &format!(
                "operator {operator} u32::{name}(left: u32, right: u32) -> u32; machine value(left: u32, right: u32) -> u32 {{ left {operator} right }}"
            ),
            true,
        );
    }
    check(
        "operator != i32::different(left: i32, right: i32) -> bool; machine value(divisor: i32) -> i32 requires divisor != 0 { 12i32 / divisor }",
        false,
    );
}

#[test]
fn runtime_divisor_preserves_call_actual_bounds_through_widening() {
    for operator in ["/", "%"] {
        for (lower, accepted) in [(1, true), (0, false)] {
            check(
                &format!(
                    "machine widen(value: u32) -> u64 [0..=4294967295] {{ value as u64 }}
                machine value(input: u64, divisor: u32) -> u64 {{
                    let bounded: u32 = clamp(divisor, {lower}, 4294967295);
                    let wide: u64 = widen(bounded);
                    input {operator} wide
                }}"
                ),
                accepted,
            );
        }
        check(
            &format!(
                "machine widen(value: u32) -> u64 [0..=4294967295] {{ value as u64 }}
            machine value(input: u64, divisor: u32) -> u64 {{
                let bounded: u32 = clamp(divisor, 1, 4294967295);
                let safe: u64 = widen(bounded);
                let first: u64 = input {operator} safe;
                let unsafe_divisor: u64 = widen(0u32);
                first {operator} unsafe_divisor
            }}"
            ),
            false,
        );
        // A signature with the same carrier does not promise that its
        // result equals its input. The current body must supply the bound.
        check(
            &format!(
                "machine widen(value: u32) -> u64 [0..=4294967295] {{ 0u64 }}
            machine value(input: u64, divisor: u32) -> u64 {{
                let bounded: u32 = clamp(divisor, 1, 4294967295);
                input {operator} widen(bounded)
            }}"
            ),
            false,
        );
    }
}
