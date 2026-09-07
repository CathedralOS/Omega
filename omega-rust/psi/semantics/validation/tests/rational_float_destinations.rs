use numerics::literals::{FloatFormat, FloatLiteral};
use source_files_to_tokens::Lexer;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

fn typed(source: &str) -> TypedTrees {
    let mut program = unlanded(source);
    validation::land_float_literal_destinations(&mut program);
    program
}

fn unlanded(source: &str) -> TypedTrees {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize rational return");
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse rational return");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("resolve rational return");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type rational return")
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
fn anonymous_numeric_quotients_round_once_at_a_float_return() {
    for (format, destination) in [(FloatFormat::F32, "f32"), (FloatFormat::F64, "f64")] {
        for (expression, expected) in [
            ("7 / 2.0 / 2", 1.75),
            ("7.0 / 2 / 2.0", 1.75),
            ("7 / 2 + 0.5", 4.0),
            ("7 / 2 / 2", 1.75),
            ("7 / 2 * 2", 7.0),
            ("7 / -2", -3.5),
            ("-7 / -2", 3.5),
            ("4097 / 4096 * 4096", 4097.0),
            ("0 / 7", 0.0),
        ] {
            let source = format!("machine value() -> {destination} {{ {expression} }}");
            let program = typed(&source);
            let literal =
                returned_float(&program).expect("one landed float, not typed integer division");
            assert_eq!(literal.landing(), Some(format), "{source}");
            assert_eq!(literal.landed_f64(), expected, "{source}");
            validation::validate_program(&program)
                .unwrap_or_else(|errors| panic!("{source}: {errors:?}"));
        }
    }
}

#[test]
fn exact_integer_arithmetic_avoids_intermediate_float_rounding() {
    for (destination, expression, expected) in [
        (
            "f32",
            "(16777219 * 1000000000000000000000 - 1) / 2000000000000000000000",
            8388609.0,
        ),
        ("f32", "16777217 - 16777216", 1.0),
        ("f32", "16777217.0 - 16777216", 1.0),
        ("f32", "16777217 / 2.0 + 1 / 18014398509481984", 8388609.0),
        ("f64", "9007199254740993 - 9007199254740992", 1.0),
        ("f64", "9007199254740993.0 - 9007199254740992", 1.0),
        (
            "f64",
            "(9007199254740993 * 10 + 1) / 10",
            9007199254740994.0,
        ),
    ] {
        let source = format!("machine value() -> {destination} {{ {expression} }}");
        let program = typed(&source);
        assert_eq!(
            returned_float(&program)
                .expect("exact landing")
                .landed_f64(),
            expected,
            "{source}"
        );
        validation::validate_program(&program)
            .unwrap_or_else(|errors| panic!("{source}: {errors:?}"));
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
fn boolean_destinations_cannot_hide_anonymous_zero_division() {
    for expression in ["1 / 0.0 > 0", "0.0 / 0 == 0", "7.0 / (2 - 2.0) != 1"] {
        let source = format!("machine value() -> bool {{ {expression} }}");
        let program = typed(&source);
        let errors = validation::validate_program(&program).expect_err("invalid anonymous value");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("anonymous division by zero")),
            "{source}: {errors:?}"
        );
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
fn a_shared_large_operand_keeps_its_other_runtime_width_obligation() {
    let mut program = unlanded(
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
