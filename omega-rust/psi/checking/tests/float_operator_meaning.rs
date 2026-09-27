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
    for (target, expression) in [("f64", "(7.0 / 2.0) + 1.0"), ("bool", "7.0 / 2.0 == 3.5")] {
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
