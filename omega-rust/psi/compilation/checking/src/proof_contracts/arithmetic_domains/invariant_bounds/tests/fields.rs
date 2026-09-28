use super::{immutable_integer_expression_bounds, query};
use typed_trees::statement::StatementNode;

#[test]
fn direct_fields_and_scalar_operands_share_exact_arithmetic_bounds() {
    for (expression, expected) in [
        ("input.value", (8, 12)),
        ("input.value + input.divisor", (10, 16)),
        ("input.value - input.divisor", (4, 10)),
        ("input.value * input.divisor", (16, 48)),
        ("input.value / input.divisor", (2, 6)),
        ("input.value % input.divisor", (0, 3)),
        ("input.value + scalar", (10, 16)),
        ("input.value - scalar", (4, 10)),
        ("scalar * input.value", (16, 48)),
        ("input.value / scalar", (2, 6)),
        ("input.value % scalar", (0, 3)),
        ("1 + input.value", (9, 13)),
        ("20u16 - input.value", (8, 12)),
        ("2u16 * input.value", (16, 24)),
        ("24u16 / input.value", (2, 3)),
        ("24u16 % input.value", (0, 11)),
        ("(input.value + input.divisor) * 2", (20, 32)),
    ] {
        assert_eq!(
            query(&format!(
                "data Inputs {{ value: u16 [8..=12]; divisor: u16 [2..=4]; }}
                 machine value(input: Inputs, scalar: u16 [2..=4]) -> u16 {{ {expression} }}"
            )),
            Some(expected),
            "{expression}"
        );
    }
}

#[test]
fn field_arithmetic_lands_completed_anonymous_rationals_once() {
    for (expression, expected) in [
        ("input.value + (3 / 2) * 2", Some((11, 15))),
        ("((3 / 2) * 2) + input.value", Some((11, 15))),
        ("input.value + 3 / 2", None),
        ("input.value + 1 / 0", None),
        ("input.value % 256", None),
        ("input.value % -1", None),
        ("input.value + 1u16", None),
        ("input.value + 1u8", Some((9, 13))),
    ] {
        assert_eq!(
            query(&format!(
                "data Input {{ value: u8 [8..=12]; }}
                 machine value(input: Input) -> u8 {{ {expression} }}"
            )),
            expected,
            "{expression}"
        );
    }
}

#[test]
fn same_spelled_foreign_state_parameters_cannot_supply_field_bounds() {
    let program = crate::front_end::typed_program(
        "data Input { value: u16 [0..=5]; }
         machine first(input: Input) -> u16 { input.value + 1 }
         machine second(input: Input) -> u16 { input.value + 1 }",
    );
    let first = &program.machines()[0];
    let second = &program.machines()[1];
    let first_state = &program.machine_states(first)[0];
    let second_state = &program.machine_states(second)[0];
    let StatementNode::Expression(expression) = program
        .statement_table
        .statements(first_state.statement_nodes)[0]
    else {
        panic!("field arithmetic expression");
    };
    assert_eq!(
        immutable_integer_expression_bounds(&program, first, first_state, expression),
        Some((1, 6))
    );
    for (machine, state) in [(second, second_state), (second, first_state)] {
        assert_eq!(
            immutable_integer_expression_bounds(&program, machine, state, expression),
            None,
            "both the parameter and its state must belong to the querying owner"
        );
    }
}
