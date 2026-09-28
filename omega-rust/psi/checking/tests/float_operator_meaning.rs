use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;

fn returned(program: &TypedTrees) -> ExpressionHandle {
    let machine = program.machines().first().expect("machine");
    let state = program.machine_states(machine).first().expect("entry");
    let [StatementNode::Expression(expression)] =
        program.statement_table.statements(state.statement_nodes)
    else {
        panic!("one return");
    };
    *expression
}

#[test]
fn builtin_parents_do_not_fold_through_an_authored_child() {
    // Suffixes establish floating operands before operator selection. Unsuffixed
    // decimals would remain exact anonymous values despite visible declarations.
    for (target, expression) in [
        ("f64", "(7.0f64 / 2.0f64) + 1.0"),
        ("bool", "7.0f64 / 2.0f64 == 3.5"),
    ] {
        let source = format!(
            "operator / f64::authored(left: f64, right: f64) -> f64;
             machine value() -> {target} {{ {expression} }}"
        );
        let mut program = crate::front_end::typed_program(&source);
        let root = returned(&program);
        let before = program.expression_table.expression(root).clone();
        validation::land_float_literal_destinations(&mut program);
        assert_eq!(
            program.expression_table.expression(root),
            &before,
            "{source}"
        );
        assert!(matches!(before, ExpressionNode::Binary(_)));
    }
}

#[test]
fn builtin_float_arithmetic_and_comparisons_still_fold_exactly() {
    for (operator, expected) in [("+", 9.0), ("-", 5.0), ("*", 14.0), ("/", 3.5)] {
        let mut program = crate::front_end::typed_program(&format!(
            "machine value() -> f64 {{ 7.0 {operator} 2.0 }}"
        ));
        let root = returned(&program);
        validation::land_float_literal_destinations(&mut program);
        let ExpressionNode::Float(literal) = program.expression_table.expression(root) else {
            panic!("builtin arithmetic must fold: {operator}");
        };
        assert_eq!(literal.landed_f64(), expected);
    }
    for (operator, expected) in [
        ("==", false),
        ("!=", true),
        ("<", false),
        ("<=", false),
        (">", true),
        (">=", true),
    ] {
        let mut program = crate::front_end::typed_program(&format!(
            "machine value() -> bool {{ 7.0 {operator} 2.0 }}"
        ));
        let root = returned(&program);
        validation::land_float_literal_destinations(&mut program);
        assert_eq!(
            program.expression_table.expression(root),
            &ExpressionNode::Boolean(expected)
        );
    }
}

#[test]
fn anonymous_subtraction_keeps_exact_meaning_with_visible_float_token_machines() {
    let program = crate::front_end::typed_program(
        "pub boundary machine - Float::subtract(left: f32, right: f32) -> f32;
         pub boundary machine - Float::subtract(left: f64, right: f64) -> f64;
         machine value() -> i32 { 0 - 64 }",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .expect("value machine");
    let state = &program.machine_states(machine)[0];
    let [StatementNode::Expression(expression)] =
        program.statement_table.statements(state.statement_nodes)
    else {
        panic!("one anonymous return");
    };
    // Both primitive declarations really participate in wildcard discovery;
    // their presence must not select a format for the anonymous operands.
    assert_eq!(
        typed_trees::operator::resolve_spelling_for_operands(
            &program,
            language_core::OperatorSpelling::Subtract,
            &[None, None],
            program.expression_table.source_span(*expression),
        )
        .len(),
        2,
    );
    assert_eq!(
        program.closed_integer_expression_value(*expression),
        Some(numerics::bignum::BigInt::from_i64(-64)),
    );
}
