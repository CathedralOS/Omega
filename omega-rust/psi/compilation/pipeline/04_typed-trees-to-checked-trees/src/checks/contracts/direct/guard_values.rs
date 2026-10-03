use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};

pub(super) mod denotations;

/// Branch truth is structural evidence, not a display-label substitution. This
/// deliberately shares the expression table's symbol-aware equality relation.
pub(super) fn proves(
    program: &TypedTrees,
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
        &|left, right| {
            program
                .expression_table
                .expressions_structurally_equal(left, right)
        },
    )
}

pub(in crate::checks::contracts) fn proves_with(
    program: &TypedTrees,
    guard: ExpressionHandle,
    guard_value: bool,
    required: ExpressionHandle,
    required_value: bool,
    equal: &impl Fn(ExpressionHandle, ExpressionHandle) -> bool,
) -> bool {
    if let ExpressionNode::Boolean(value) = program.expression_table.expression(guard)
        && *value != guard_value
    {
        return true;
    }
    if let ExpressionNode::Boolean(value) = program.expression_table.expression(required) {
        return *value == required_value;
    }
    if let ExpressionNode::Unary(unary) = program.expression_table.expression(guard)
        && unary.operator == UnaryOperator::LogicalNot
        && crate::authored_selections::typed_operator_has_no_authored_selection(program, guard)
    {
        return proves_with(
            program,
            unary.operand,
            !guard_value,
            required,
            required_value,
            equal,
        );
    }
    if let ExpressionNode::Unary(unary) = program.expression_table.expression(required)
        && unary.operator == UnaryOperator::LogicalNot
        && crate::authored_selections::typed_operator_has_no_authored_selection(program, required)
    {
        return proves_with(
            program,
            guard,
            guard_value,
            unary.operand,
            !required_value,
            equal,
        );
    }
    if let Some((operand, value)) = boolean_comparison(program, guard, guard_value) {
        return proves_with(program, operand, value, required, required_value, equal);
    }
    if let Some((operand, value)) = boolean_comparison(program, required, required_value) {
        return proves_with(program, guard, guard_value, operand, value, equal);
    }
    if equal(guard, required) {
        return guard_value == required_value;
    }
    if let ExpressionNode::Binary(binary) = program.expression_table.expression(guard)
        && ((binary.operator == BinaryOperator::And && guard_value)
            || (binary.operator == BinaryOperator::Or && !guard_value))
    {
        return proves_with(
            program,
            binary.left,
            guard_value,
            required,
            required_value,
            equal,
        ) || proves_with(
            program,
            binary.right,
            guard_value,
            required,
            required_value,
            equal,
        );
    }
    if let ExpressionNode::Binary(binary) = program.expression_table.expression(required) {
        let left = || {
            proves_with(
                program,
                guard,
                guard_value,
                binary.left,
                required_value,
                equal,
            )
        };
        let right = || {
            proves_with(
                program,
                guard,
                guard_value,
                binary.right,
                required_value,
                equal,
            )
        };
        match (binary.operator, required_value) {
            (BinaryOperator::And, true) | (BinaryOperator::Or, false) => return left() && right(),
            (BinaryOperator::Or, true) | (BinaryOperator::And, false) => return left() || right(),
            _ => {}
        }
    }
    false
}

fn boolean_comparison(
    program: &TypedTrees,
    expression: ExpressionHandle,
    value: bool,
) -> Option<(ExpressionHandle, bool)> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
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
        if let ExpressionNode::Boolean(literal) = program.expression_table.expression(literal) {
            return Some((
                operand,
                (*literal == value) == (binary.operator == BinaryOperator::Equal),
            ));
        }
    }
    None
}
