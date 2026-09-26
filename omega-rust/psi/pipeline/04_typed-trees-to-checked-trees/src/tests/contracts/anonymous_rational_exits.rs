use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::contracts::parse_typed_trees;

fn accepts(source: &str) -> checked_trees::CheckedTrees {
    lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

fn rejects(source: &str) {
    match lower_typed_trees(parse_typed_trees(source), &CheckingRequest::settled()) {
        Ok(_) => panic!("invalid rational landing or guarantee was accepted: {source}"),
        Err(diagnostics) => assert!(!diagnostics.is_empty(), "{source}"),
    }
}

fn guaranteed(expression: &str, target: &str, expected: i64) -> String {
    format!("machine value() -> {target} ensures result == {expected} {{ {expression} }}")
}

#[test]
fn integer_casts_share_the_exact_anonymous_value_with_range_validation() {
    for (expression, expected) in [
        ("7 / 2 * 2", 7),
        ("7 / 2.0 * 2", 7),
        ("7.0 / 2 * 2", 7),
        ("0.1 + 0.9", 1),
        ("9007199254740993.0 - 9007199254740992", 1),
    ] {
        accepts(&format!(
            "machine value() -> i32 [{expected}..={expected}] {{ ({expression}) as i32 }}"
        ));
        let wrong = expected - 1;
        rejects(&format!(
            "machine value() -> i32 [{wrong}..={wrong}] {{ ({expression}) as i32 }}"
        ));
        let cast = format!("({expression}) as i32");
        accepts(&guaranteed(&cast, "i32", expected));
        rejects(&guaranteed(&cast, "i32", wrong));
    }
    for expression in ["7 / 2", "7.0 / 2", "7 / 2.0"] {
        rejects(&format!(
            "machine value() -> i32 {{ ({expression}) as i32 }}"
        ));
    }
}
