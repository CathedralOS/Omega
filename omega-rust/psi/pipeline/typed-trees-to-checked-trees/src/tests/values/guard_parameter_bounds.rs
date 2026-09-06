use super::*;

fn check(source: &str, accepted: bool) {
    match lower_typed_trees(typed_trees(source)) {
        Ok(_) => assert!(accepted, "unproved arithmetic accepted: {source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{source}\n{diagnostics:#?}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("may overflow")),
                "expected arithmetic rejection: {source}\n{diagnostics:#?}"
            );
        }
    }
}

#[test]
fn ordered_unsigned_parameter_proves_a_nonzero_operand() {
    for condition in ["n > floor", "floor < n", "!(n <= floor)"] {
        check(
            &format!(
                "machine decrement(floor: u64, n: u64) -> u64 {{
            transition {condition} {{ true -> finish(n - 1) false -> n }}
            state finish(result: u64) -> u64 {{ result }}
        }}"
            ),
            true,
        );
    }
    check(
        "machine decrement(floor: u64, n: u64) -> u64 {
        transition n <= floor { true -> n false -> (n - 1) }
    }",
        true,
    );
}

#[test]
fn comparison_polarity_and_strictness_do_not_invent_a_nonzero_operand() {
    for condition in ["n >= floor", "n < floor", "n <= floor"] {
        check(
            &format!(
                "machine decrement(floor: u64, n: u64) -> u64 {{
            transition {condition} {{ true -> (n - 1) false -> n }}
        }}"
            ),
            false,
        );
    }
}

#[test]
fn authored_ordering_cannot_supply_primitive_integer_bounds() {
    check(
        "operator > u64::replacement(left: u64, right: u64) -> bool;
        machine decrement(floor: u64, n: u64) -> u64 {
            transition n > floor { true -> (n - 1) false -> n }
        }",
        false,
    );
    check(
        "operator > f64::unrelated(left: f64, right: f64) -> bool;
        machine decrement(floor: u64, n: u64) -> u64 {
            transition n > floor { true -> (n - 1) false -> n }
        }",
        true,
    );
}
