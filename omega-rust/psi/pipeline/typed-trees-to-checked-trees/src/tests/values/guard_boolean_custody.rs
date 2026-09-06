use super::*;

fn check(source: &str, accepted: bool) {
    match lower_typed_trees(typed_trees(source)) {
        Ok(_) => assert!(
            accepted,
            "authored Boolean equality supplied a bound: {source}"
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
fn authored_boolean_wrappers_do_not_prove_their_operands() {
    for (spelling, condition) in [
        ("==", "(n > 0) == true"),
        ("==", "true == (n > 0)"),
        ("!=", "(n > 0) != false"),
        ("!=", "false != (n > 0)"),
    ] {
        check(
            &format!(
                "operator {spelling} bool::custom(left: bool, right: bool) -> bool;
            machine decrement(n: u64) -> u64 {{
                transition {condition} {{ true -> (n - 1) false -> n }}
            }}"
            ),
            false,
        );
    }
}

#[test]
fn authored_boolean_wrappers_do_not_prove_false_arms_or_fallthrough() {
    for (spelling, condition) in [
        ("==", "(n > 0) == false"),
        ("==", "false == (n > 0)"),
        ("!=", "(n > 0) != true"),
        ("!=", "true != (n > 0)"),
    ] {
        for body in [
            format!("transition {condition} {{ true -> n false -> (n - 1) }}"),
            format!("transition {{ ({condition}) -> n _ -> (n - 1) }}"),
        ] {
            check(
                &format!(
                    "operator {spelling} bool::custom(left: bool, right: bool) -> bool;
                machine decrement(n: u64) -> u64 {{ {body} }}"
                ),
                false,
            );
        }
    }
}

#[test]
fn generated_boolean_arm_comparisons_do_not_bypass_equality_selection() {
    for body in [
        "transition n > 0 { true -> (n - 1) false -> n }",
        "transition n > 0 { false -> n true -> (n - 1) }",
    ] {
        check(
            &format!(
                "operator == bool::custom(left: bool, right: bool) -> bool;
            machine decrement(n: u64) -> u64 {{ {body} }}"
            ),
            false,
        );
    }
}

#[test]
fn builtin_boolean_wrappers_keep_nested_polarity_and_snapshot_bounds() {
    for condition in [
        "(n > 0) == true",
        "true == (n > 0)",
        "(n > 0) != false",
        "false != (n > 0)",
        "(!(n > 0)) == false",
        "true != (!(n > 0))",
        "!((n > 0) == false)",
    ] {
        for (parameters, prefix) in [
            ("n: u64", ""),
            ("mut n: u64", ""),
            ("input: u64", "let n: u64 = input;"),
        ] {
            check(
                &format!(
                    "machine decrement({parameters}) -> u64 {{ {prefix}
                transition {condition} {{ true -> (n - 1) false -> n }}
            }}"
                ),
                true,
            );
        }
    }
}

#[test]
fn builtin_boolean_false_arms_and_fallthrough_keep_bounds() {
    for condition in [
        "(n > 0) == false",
        "false == (n > 0)",
        "(n > 0) != true",
        "true != (n > 0)",
    ] {
        for body in [
            format!("transition {condition} {{ true -> n false -> (n - 1) }}"),
            format!("transition {{ ({condition}) -> n _ -> (n - 1) }}"),
        ] {
            check(
                &format!("machine decrement(n: u64) -> u64 {{ {body} }}"),
                true,
            );
        }
    }
}

#[test]
fn unrelated_equality_preserves_builtin_boolean_wrappers() {
    for condition in ["n > 0", "(n > 0) == true", "false != (n > 0)"] {
        check(
            &format!(
                "operator == f64::equal(left: f64, right: f64) -> bool;
            operator != f64::not_equal(left: f64, right: f64) -> bool;
            machine decrement(n: u64) -> u64 {{
                transition {condition} {{ true -> (n - 1) false -> n }}
            }}"
            ),
            true,
        );
    }
}
