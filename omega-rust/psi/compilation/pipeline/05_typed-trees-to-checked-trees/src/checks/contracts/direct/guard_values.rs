use crate::flow::expressions;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};

pub(super) mod denotations;

/// Branch truth is structural evidence, not a display-label substitution. This
/// deliberately shares the expression table's symbol-aware equality relation.
pub(super) fn proves(
    program: &dyn ::validation::CheckingInput,
    guard: ExpressionHandle,
    guard_value: bool,
    required: ExpressionHandle,
    required_value: bool,
) -> bool {
    proves_with(
        program,
        guard,
        guard_value,
        required,
        required_value,
        &|left, right| expressions(program).expressions_structurally_equal(left, right),
    )
}

pub(in crate::checks::contracts) fn proves_with(
    program: &dyn ::validation::CheckingInput,
    guard: ExpressionHandle,
    guard_value: bool,
    required: ExpressionHandle,
    required_value: bool,
    equal: &impl Fn(ExpressionHandle, ExpressionHandle) -> bool,
) -> bool {
    macro_rules! prove_guard {
        ($guard:expr, $guard_value:expr, $required:expr, $required_value:expr $(,)?) => {
            proves_with(
                program,
                $guard,
                $guard_value,
                $required,
                $required_value,
                equal,
            )
        };
    }
    if let ExpressionNode::Boolean(value) = program.expression_table.expression(guard)
        && *value != guard_value
    {
        return true;
    }
    if let Some(value) = program.expression_table.boolean_node(required) {
        return *value == required_value;
    }
    if let ExpressionNode::Unary(unary) = program.expression_table.expression(guard)
        && unary.operator == UnaryOperator::LogicalNot
        && crate::authored_selections::typed_operator_has_no_authored_selection(program, guard)
    {
        return prove_guard!(unary.operand, !guard_value, required, required_value);
    }
    if let ExpressionNode::Unary(unary) = program.expression_table.expression(required)
        && unary.operator == UnaryOperator::LogicalNot
        && crate::authored_selections::typed_operator_has_no_authored_selection(program, required)
    {
        return prove_guard!(guard, guard_value, unary.operand, !required_value);
    }
    if let Some((operand, value)) = boolean_comparison(program, guard, guard_value) {
        return prove_guard!(operand, value, required, required_value);
    }
    if let Some((operand, value)) = boolean_comparison(program, required, required_value) {
        return prove_guard!(guard, guard_value, operand, value);
    }
    if equal(guard, required) {
        return guard_value == required_value;
    }
    if let Some(binary) = program.expression_table.binary_node(guard)
        && ((binary.operator == BinaryOperator::And && guard_value)
            || (binary.operator == BinaryOperator::Or && !guard_value))
    {
        return prove_guard!(binary.left, guard_value, required, required_value)
            || prove_guard!(binary.right, guard_value, required, required_value);
    }
    if let Some(binary) = program.expression_table.binary_node(required) {
        let left = || prove_guard!(guard, guard_value, binary.left, required_value);
        let right = || prove_guard!(guard, guard_value, binary.right, required_value);
        match (binary.operator, required_value) {
            (BinaryOperator::And, true) | (BinaryOperator::Or, false) => return left() && right(),
            (BinaryOperator::Or, true) | (BinaryOperator::And, false) => return left() || right(),
            _ => {}
        }
    }
    false
}

fn boolean_comparison(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    value: bool,
) -> Option<(ExpressionHandle, bool)> {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return None;
    };
    if !matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) || !crate::authored_selections::typed_operator_has_no_authored_selection(
        program, expression,
    ) {
        return None;
    }
    for (operand, literal) in [(binary.left, binary.right), (binary.right, binary.left)] {
        if let Some(literal) = program.expression_table.boolean_node(literal) {
            return Some((
                operand,
                (*literal == value) == (binary.operator == BinaryOperator::Equal),
            ));
        }
    }
    None
}
