use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::name::Identifier;

use super::patterns;

/// Proves a self-loop terminates under a struct-view measure that projects a
/// single field, e.g. `measure Card::PowerOrder(card: Card) -> usize { card.power }`
/// used as `terminates by card -> Card::PowerOrder`.
///
/// The decreasing value is the whole struct value; each recursive call argument is
/// a struct literal whose projected field must strictly decrease relative to the
/// projected field of the decreasing value, guarded by `<decrease>.field > 0`.
pub(super) fn state_has_proven_self_loop(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    decreases: ExpressionHandle,
    field: &Identifier,
) -> bool {
    let Some((parameter, argument_index)) =
        patterns::parameter_and_argument_index_matched_by_expression(program, state, decreases)
    else {
        return false;
    };

    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| patterns::guarded_self_loop(program, state, statement))
        .any(|self_loop| {
            let Some(argument) = self_loop.arguments.get(argument_index).copied() else {
                return false;
            };
            guard_is_positive_member(program, self_loop.guard, parameter, field)
                && argument_rebuilds_with_field_minus_one(program, argument, parameter, field)
        })
}

fn guard_is_positive_member(
    program: &typed_trees::TypedTrees,
    guard: ExpressionHandle,
    parameter: &typed_trees::signature::StateParameter,
    field: &Identifier,
) -> bool {
    let normalized = patterns::normalize_boolean_guard(program, guard);
    let ExpressionNode::Binary(binary) = program.expression_table.expression(normalized) else {
        return false;
    };
    matches!(binary.operator, BinaryOperator::Greater)
        && patterns::expression_is_parameter_member(program, binary.left, parameter, field.as_str())
        && matches!(
            program.expression_table.expression(binary.right),
            ExpressionNode::Integer(literal) if literal.value_i64() == Some(0)
        )
}

fn argument_rebuilds_with_field_minus_one(
    program: &typed_trees::TypedTrees,
    argument: ExpressionHandle,
    parameter: &typed_trees::signature::StateParameter,
    field: &Identifier,
) -> bool {
    let ExpressionNode::StructLiteral(struct_literal) =
        program.expression_table.expression(argument)
    else {
        return false;
    };

    program
        .expression_table
        .struct_fields(struct_literal.fields)
        .iter()
        .any(|literal_field| {
            literal_field.name.as_str() == field.as_str()
                && field_value_is_member_minus_one(program, literal_field.value, parameter, field)
        })
}

fn field_value_is_member_minus_one(
    program: &typed_trees::TypedTrees,
    value: ExpressionHandle,
    parameter: &typed_trees::signature::StateParameter,
    field: &Identifier,
) -> bool {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(value) else {
        return false;
    };
    matches!(binary.operator, BinaryOperator::Subtract)
        && patterns::expression_is_parameter_member(program, binary.left, parameter, field.as_str())
        && matches!(
            program.expression_table.expression(binary.right),
            ExpressionNode::Integer(literal) if literal.value_i64() == Some(1)
        )
}
