use super::parse_typed_trees;
use crate::CheckingRequest;
use crate::lower_typed_trees;
use typed_trees::TypedTrees;

fn check(source: &str, accepted: bool) {
    check_program(parse_typed_trees(source), accepted, source);
}

fn check_program(program: TypedTrees, accepted: bool, description: &str) {
    match lower_typed_trees(program, &CheckingRequest::settled()) {
        Ok(_) => assert!(accepted, "unproved result bounds accepted: {description}"),
        Err(diagnostics) => {
            assert!(!accepted, "{description}: {diagnostics:#?}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("cannot prove ensures contract")),
                "expected an exit proof rejection for {description}: {diagnostics:#?}"
            );
        }
    }
}

#[test]
fn negated_comparisons_require_refutation_not_failed_proof() {
    for (guarantee, accepted) in [
        ("!(result < 2)", true),
        ("!(2 > result)", true),
        ("!(result <= 1)", true),
        ("!(result > 4)", true),
        ("!(result >= 5)", true),
        ("!(result == 1)", true),
        ("!!(result >= 2)", true),
        ("!(result < 3)", false),
        ("!(result <= 3)", false),
        ("!(result > 3)", false),
        ("!(result >= 3)", false),
        ("!(result == 3)", false),
        ("!(result != 3)", false),
        ("!(result >= 2)", false),
    ] {
        check(
            &format!(
                "machine choose(value: u64 [2..=4]) -> u64
                 ensures {guarantee} {{ value }}"
            ),
            accepted,
        );
    }
    check(
        "machine choose(value: u64 [3..=3]) -> u64
         ensures !(result != 3) { value }",
        true,
    );
}

#[test]
fn selected_comparisons_and_arithmetic_supply_no_builtin_result_bounds() {
    for (declaration, returned, guarantee) in [
        (
            "operator >= u64::custom(left: u64, right: u64) -> bool;",
            "value",
            "result >= 2",
        ),
        (
            "operator < u64::custom(left: u64, right: u64) -> bool;",
            "value",
            "1 < result",
        ),
        (
            "operator + u64::custom(left: u64, right: u64) -> u64;",
            "value + 1",
            "result >= 3",
        ),
        (
            "operator < u64::custom(left: u64, right: u64) -> bool;",
            "value",
            "!(result < 2)",
        ),
    ] {
        check(
            &format!(
                "{declaration}
                 machine choose(value: u64 [2..=4]) -> u64
                 ensures {guarantee} {{ {returned} }}"
            ),
            false,
        );
    }
}
