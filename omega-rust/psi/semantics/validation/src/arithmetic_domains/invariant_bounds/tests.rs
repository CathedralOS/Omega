use super::*;

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

fn query(source: &str) -> Option<(i64, i64)> {
    let program = typed(source);
    let machine = program.machines().first().unwrap();
    let state = program.machine_states(machine).first().unwrap();
    let typed_trees::statement::StatementNode::Expression(expression) = program
        .statement_table
        .statements(state.statement_nodes)
        .last()
        .unwrap()
    else {
        panic!("value expression");
    };
    immutable_integer_expression_bounds(&program, machine, state, *expression)
}

#[test]
fn immutable_arithmetic_uses_existing_interval_transfers() {
    for (expression, expected) in [
        ("input % 256u16", (0, 255)),
        ("(input % 128u16) + 1u16", (1, 128)),
        ("(input % 128u16) * 2u16", (0, 254)),
    ] {
        assert_eq!(
            query(&format!(
                "machine value(input: u16) -> u16 {{ {expression} }}"
            )),
            Some(expected)
        );
    }
    assert_eq!(
        query("machine value(input: u16 [0..=255]) -> u16 { input }"),
        Some((0, 255))
    );
    assert_eq!(
        query(
            "machine value(input: i16 [1..=10], divisor: i16 [-2..=-1]) -> i16 { input / divisor }"
        ),
        Some((-10, 0))
    );
}

#[test]
fn mutable_values_initializers_policies_and_oversize_divisors_stay_unknown() {
    for source in [
        "machine value(mut input: u16) -> u16 { input % 256u16 }",
        "machine value(input: u16) -> u16 { let saved: u16 = input; saved % 256u16 }",
        "machine value(input: u16 in Wrapping) -> u16 { input % 256u16 }",
        "machine value(input: i64) -> i64 { input % -9223372036854775808i64 }",
        "machine value(input: u16) -> u16 { input % 0u16 }",
    ] {
        assert_eq!(query(source), None, "{source}");
    }
}

#[test]
fn another_states_same_spelled_parameter_has_no_bound() {
    let program = typed(
        "machine first(input: u16) -> u16 { input % 256u16 } machine second(input: u16) -> u16 { input }",
    );
    let first = &program.machines()[0];
    let second = &program.machines()[1];
    let first_state = &program.machine_states(first)[0];
    let second_state = &program.machine_states(second)[0];
    let typed_trees::statement::StatementNode::Expression(expression) = program
        .statement_table
        .statements(first_state.statement_nodes)[0]
    else {
        panic!("expression");
    };
    assert_eq!(
        immutable_integer_expression_bounds(&program, second, second_state, expression),
        None
    );
}

#[test]
fn comparison_bounds_keep_unsigned_floors_without_representable_ceilings() {
    let program = typed("machine compare(left: u64, right: u64) -> bool { left > right }");
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let typed_trees::statement::StatementNode::Expression(expression) =
        program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("comparison expression");
    };
    let (left, right) = builtin_comparison_intervals(&program, machine, state, expression)
        .expect("both exact immutable unsigned parameters");
    assert_eq!(
        left,
        Interval {
            low: Some(0),
            high: None
        }
    );
    assert_eq!(right, left);
}

#[test]
fn comparison_bounds_do_not_rebind_a_foreign_parameter_by_spelling() {
    let program = typed(
        "machine first(left: u64, right: u64) -> bool { left > right }
        machine second(left: u64, right: u64) -> bool { left > right }",
    );
    let first = &program.machines()[0];
    let second = &program.machines()[1];
    let first_state = &program.machine_states(first)[0];
    let second_state = &program.machine_states(second)[0];
    let typed_trees::statement::StatementNode::Expression(expression) = program
        .statement_table
        .statements(first_state.statement_nodes)[0]
    else {
        panic!("comparison expression");
    };
    assert!(builtin_comparison_intervals(&program, second, second_state, expression).is_none());
}
