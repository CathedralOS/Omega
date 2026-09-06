use super::*;

fn check(source: &str, accepted: bool) {
    match lower_typed_trees(typed_trees(source)) {
        Ok(_) => assert!(
            accepted,
            "authored comparison supplied a primitive bound: {source}"
        ),
        Err(diagnostics) => {
            assert!(!accepted, "{source}\n{diagnostics:#?}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("may overflow")),
                "{source}\n{diagnostics:#?}"
            );
        }
    }
}

#[test]
fn singleton_and_literal_orderings_require_builtin_meaning() {
    for (spelling, condition) in [
        (">", "n > floor"),
        (">", "n > 0"),
        ("<", "floor < n"),
        ("<", "0 < n"),
    ] {
        let source = format!(
            "operator {spelling} u64::custom(left: u64, right: u64) -> bool;
            machine decrement(floor: u64 [0..=0], n: u64) -> u64 {{
                transition {condition} {{ true -> (n - 1) false -> n }}
            }}"
        );
        check(&source, false);
    }
}

#[test]
fn false_arm_and_fallthrough_cannot_negate_an_authored_ordering() {
    for (spelling, condition) in [("<=", "n <= 0"), (">=", "0 >= n"), ("<=", "n <= floor")] {
        for body in [
            format!("transition {condition} {{ true -> n false -> (n - 1) }}"),
            format!("transition {{ {condition} -> n _ -> (n - 1) }}"),
        ] {
            check(
                &format!(
                    "operator {spelling} u64::custom(left: u64, right: u64) -> bool;
                machine decrement(floor: u64 [0..=0], n: u64) -> u64 {{ {body} }}"
                ),
                false,
            );
        }
    }
}

#[test]
fn unrelated_ordering_does_not_hide_builtin_literal_or_singleton_bounds() {
    for (spelling, condition, positive) in [
        (">", "n > 0", true),
        (">", "n > floor", true),
        ("<", "0 < n", true),
        ("<=", "n <= 0", false),
    ] {
        let arms = if positive {
            "true -> (n - 1) false -> n"
        } else {
            "true -> n false -> (n - 1)"
        };
        check(&format!("operator {spelling} f64::unrelated(left: f64, right: f64) -> bool;
            machine decrement(floor: u64 [0..=0], n: u64) -> u64 {{ transition {condition} {{ {arms} }} }}"), true);
    }
}

#[test]
fn builtin_mutable_parameters_and_local_places_keep_snapshot_narrowing() {
    for (parameters, prefix, name) in [
        ("mut n: u64", "", "n"),
        ("input: u64", "let n: u64 = input;", "n"),
        ("input: u64", "let mut n: u64 = input;", "n"),
    ] {
        check(&format!("operator > f64::unrelated(left: f64, right: f64) -> bool;
            machine decrement({parameters}) -> u64 {{ {prefix} transition {name} > 0 {{ true -> ({name} - 1) false -> {name} }} }}"), true);
    }
}

#[test]
fn authored_ordering_on_mutable_and_local_places_does_not_supply_bounds() {
    for (parameters, prefix) in [("mut n: u64", ""), ("input: u64", "let n: u64 = input;")] {
        check(&format!("operator > u64::custom(left: u64, right: u64) -> bool;
            machine decrement({parameters}) -> u64 {{ {prefix} transition n > 0 {{ true -> (n - 1) false -> n }} }}"), false);
    }
}
