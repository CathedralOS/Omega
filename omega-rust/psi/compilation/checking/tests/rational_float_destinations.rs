use numerics::literals::FloatLiteral;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

fn typed(source: &str) -> TypedTrees {
    let mut program = crate::front_end::typed_program(source);
    validation::land_float_literal_destinations(&mut program);
    program
}

fn returned_float(program: &TypedTrees) -> Option<&FloatLiteral> {
    let machine = program.machines().first().expect("machine");
    let state = program.machine_states(machine).first().expect("entry");
    let [StatementNode::Expression(expression)] =
        program.statement_table.statements(state.statement_nodes)
    else {
        panic!("one returned expression");
    };
    match program.expression_table.expression(*expression) {
        ExpressionNode::Float(literal) => Some(literal),
        _ => None,
    }
}

#[test]
fn float_destinations_do_not_make_anonymous_zero_division_finite_or_infinite() {
    for expression in [
        "7 / 0",
        "0 / 0",
        "7 / (2 - 2)",
        "7 / 0.0",
        "0.0 / 0",
        "7.0 / (2 - 2.0)",
        "1.0 / -0.0",
    ] {
        let source = format!("machine value() -> f64 {{ {expression} }}");
        let program = typed(&source);
        assert!(
            returned_float(&program).is_none(),
            "invalid anonymous arithmetic cannot become a float: {source}"
        );
        assert!(validation::validate_program(&program).is_err(), "{source}");
    }
}

#[test]
fn typed_integer_operands_are_not_reclassified_as_anonymous_floats() {
    for expression in ["7i32 / 2", "7 / 2i32", "7i32 / 2 * 2"] {
        let source = format!("machine value() -> f64 {{ {expression} }}");
        let program = typed(&source);
        assert!(returned_float(&program).is_none(), "{source}");
        let errors = validation::validate_program(&program)
            .expect_err("typed arithmetic needs an explicit conversion");
        assert!(
            errors.iter().any(|error| error
                .message
                .contains("typed integer arithmetic cannot implicitly land")),
            "{errors:?}"
        );
    }
}

#[test]
fn decimal_float_landing_preserves_signed_zero_arithmetic() {
    for destination in ["f32", "f64"] {
        for (expression, expected) in [
            ("-0.0 + -0.0", -0.0_f64),
            ("-0.0 - 0.0", -0.0_f64),
            ("-0.0 + 0.0", 0.0_f64),
        ] {
            let source = format!("machine value() -> {destination} {{ {expression} }}");
            let program = typed(&source);
            assert_eq!(
                returned_float(&program)
                    .expect("float zero")
                    .landed_f64()
                    .to_bits(),
                expected.to_bits(),
                "{source}"
            );
        }
    }
}

#[test]
fn a_shared_large_operand_keeps_its_other_runtime_width_obligation() {
    let mut program = crate::front_end::typed_program(
        "machine value() -> f64 { 1000000000000000000000 / 1000000000000000000000 }
         machine other() { let integer: i32 = 0; }",
    );
    let shared = program.expression_table.expression_entries().find_map(|(handle, node)| {
        matches!(node, ExpressionNode::Integer(literal) if literal.text() == "1000000000000000000000").then_some(handle)
    }).expect("large anonymous operand");
    let other = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "other")
        .expect("other machine");
    let statements = program.machine_states(other)[0].statement_nodes;
    let (statement, _) = program
        .statement_table
        .iter_statements(statements)
        .next()
        .expect("local");
    let StatementNode::LocalData(local) = program.statement_table.statement_mut(statement) else {
        panic!("local initializer");
    };
    local.initial_value = shared;
    validation::land_float_literal_destinations(&mut program);
    assert_eq!(
        returned_float(&program).expect("float result").landed_f64(),
        1.0
    );
    let errors =
        validation::validate_program(&program).expect_err("the shared integer use remains invalid");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("exceeds the i64 range")),
        "{errors:?}"
    );
}
