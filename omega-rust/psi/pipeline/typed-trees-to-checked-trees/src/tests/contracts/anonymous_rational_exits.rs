use super::*;
use checked_trees::{CheckedScalarExpression, CheckedScalarExpressionRole};

fn accepts(source: &str) -> checked_trees::CheckedTrees {
    lower_typed_trees(parse_typed_trees(source))
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

fn rejects(source: &str) {
    match lower_typed_trees(parse_typed_trees(source)) {
        Ok(_) => panic!("invalid rational landing or guarantee was accepted: {source}"),
        Err(diagnostics) => assert!(!diagnostics.is_empty(), "{source}"),
    }
}

fn guaranteed(expression: &str, target: &str, expected: i64) -> String {
    format!("machine value() -> {target} ensures result == {expected} {{ {expression} }}")
}

#[test]
fn anonymous_fractional_intermediates_retain_the_exact_integral_return() {
    for (expression, target, expected) in [
        ("7 / 2 * 2", "i32", 7),
        ("(4097 / 4096) * 4096", "u32", 4097),
    ] {
        let checked = accepts(&guaranteed(expression, target, expected));
        let plans = &checked.facts.values.scalar_expressions;
        let mut returns = plans
            .source_bindings
            .iter()
            .map(|(_, binding)| binding)
            .filter(|binding| binding.role == CheckedScalarExpressionRole::Return);
        let returned = returns.next().expect("one retained scalar return");
        assert!(returns.next().is_none(), "one source return: {expression}");
        assert!(
            matches!(
                plans.expression_at(returned.state, returned.statement_ordinal, returned.role),
                Some(CheckedScalarExpression::IntegerLiteral { literal })
                    if literal.value_i64() == Some(expected)
            ),
            "the retained return must carry the exact landed value {expected}: {expression}"
        );
    }
}

#[test]
fn rational_cancellation_does_not_prove_the_truncated_integer_answer() {
    rejects(&guaranteed("7 / 2 * 2", "i32", 6));
    rejects(&guaranteed("(4097 / 4096) * 4096", "u32", 4096));
}

#[test]
fn final_fractions_cannot_land_in_integer_returns_or_storage() {
    for expression in ["7 / 2", "7 / 2 / 2", "4097 / 4096"] {
        rejects(&format!("machine value() -> i32 {{ {expression} }}"));
        rejects(&format!(
            "machine value() {{ let landed: i32 = {expression}; }}"
        ));
        rejects(&format!(
            "machine value() {{ let mut landed: i32 = 0; landed = {expression}; }}"
        ));
    }
}

#[test]
fn rational_cancellation_lands_once_at_local_and_assignment_destinations() {
    for (expression, target, expected) in [
        ("7 / 2 * 2", "i32", 7),
        ("(4097 / 4096) * 4096", "u32", 4097),
    ] {
        for body in [
            format!("let landed: {target} = {expression}; landed"),
            format!("let mut landed: {target} = 0; landed = {expression}; landed"),
        ] {
            accepts(&guaranteed(&body, target, expected));
            rejects(&guaranteed(&body, target, expected - 1));
        }
    }
}

#[test]
fn explicitly_typed_division_keeps_integer_quotients() {
    for (expression, target, expected, rational_answer) in [
        ("7i32 / 2 * 2", "i32", 6, 7),
        ("(4097u32 / 4096) * 4096", "u32", 4096, 4097),
    ] {
        accepts(&guaranteed(expression, target, expected));
        rejects(&guaranteed(expression, target, rational_answer));
    }
}

#[test]
fn a_runtime_operand_cannot_truncate_an_anonymous_fraction() {
    for expression in ["input * (4097 / 2)", "(4097 / 2) * input"] {
        rejects(&format!(
            "machine value(input: i32 [0..=1]) -> i32 {{ {expression} }}"
        ));
    }
}

#[test]
fn typed_quotient_and_exact_anonymous_cancellation_are_valid_runtime_operands() {
    for expression in [
        "input * (4097i32 / 2)",
        "(4097i32 / 2) * input",
        "input * (4097 / 2 * 2)",
    ] {
        accepts(&format!(
            "machine value(input: i32 [0..=1]) -> i32 {{ {expression} }}"
        ));
    }
}

fn returned(checked: &checked_trees::CheckedTrees) -> &CheckedScalarExpression {
    let plans = &checked.facts.values.scalar_expressions;
    let binding = plans
        .source_bindings
        .iter()
        .map(|(_, binding)| binding)
        .find(|binding| binding.role == CheckedScalarExpressionRole::Return)
        .expect("retained scalar return");
    plans
        .expression_at(binding.state, binding.statement_ordinal, binding.role)
        .expect("the return binding owns its scalar expression")
}

#[test]
fn nested_rational_operands_retain_their_exact_value_in_both_positions() {
    for (expression, anonymous_on_left) in [
        ("input * (4097 / 2 * 2)", false),
        ("(4097 / 2 * 2) * input", true),
    ] {
        let checked = accepts(&format!(
            "machine value(input: i32 [0..=1]) -> i32 {{ {expression} }}"
        ));
        let CheckedScalarExpression::IntegerBinary { left, right, .. } = returned(&checked) else {
            panic!("runtime multiplication must retain its operands: {expression}");
        };
        let operand = if anonymous_on_left { left } else { right };
        assert!(
            matches!(operand.as_ref(), CheckedScalarExpression::IntegerLiteral { literal }
            if literal.value_i64() == Some(4097))
        );
    }
}

#[test]
fn boolean_comparison_operands_land_to_the_integer_peer_not_bool() {
    for expression in ["input == (7 / 2 * 2)", "(7 / 2 * 2) == input"] {
        let checked = accepts(&format!(
            "machine value(input: i32) -> bool {{ {expression} }}"
        ));
        let CheckedScalarExpression::Boolean(comparison) = returned(&checked) else {
            panic!("retained Boolean comparison");
        };
        let checked_trees::CheckedBooleanExpression::IntegerComparison { left, right, .. } =
            comparison.as_ref()
        else {
            panic!("retained integer comparison");
        };
        assert!([left, right].iter().any(
            |operand| matches!(operand.as_ref(), CheckedScalarExpression::IntegerLiteral { literal }
            if literal.value_i64() == Some(7))
        ));
    }
    for expression in ["input == (7 / 2)", "(7 / 2) == input"] {
        rejects(&format!(
            "machine value(input: i32) -> bool {{ {expression} }}"
        ));
    }
}

#[test]
fn mixed_cancellation_must_fit_the_peer_before_the_outer_operation() {
    for expression in ["0i8 * (513 / 2 * 2)", "(513 / 2 * 2) * 0i8"] {
        rejects(&format!("machine value() -> i8 {{ {expression} }}"));
    }
}

#[test]
fn call_bearing_computations_retain_exact_rational_operands() {
    use checked_trees::CheckedScalarComputationKind;
    let checked = accepts(
        "machine sample(input: i32 [0..=1]) -> i32 [0..=1] { input }
         machine value(input: i32 [0..=1]) -> i32 { sample(input) * (4097 / 2 * 2) }",
    );
    let computations = &checked.facts.values.scalar_computations;
    assert!(
        computations.nodes.iter().any(|(_, node)| matches!(
            &node.kind,
            CheckedScalarComputationKind::Value(CheckedScalarExpression::IntegerLiteral { literal })
                if literal.value_i64() == Some(4097)
        )),
        "the call-bearing plan must materialize the exact anonymous operand"
    );
}

#[test]
fn integer_casts_share_the_exact_anonymous_value_with_range_validation() {
    accepts("machine value() -> i32 [7..=7] { (7 / 2 * 2) as i32 }");
    rejects("machine value() -> i32 [6..=6] { (7 / 2 * 2) as i32 }");
    rejects("machine value() -> i32 { (7 / 2) as i32 }");
}
