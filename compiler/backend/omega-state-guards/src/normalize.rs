use omega_checked_trees::expression::{BinaryExpression, BinaryOperator, Expression};
use std::sync::Arc;

pub(super) fn normalize_guard_expression(expression: Expression) -> Expression {
    normalize_top_level_guard_expression(expression)
}

fn normalize_top_level_guard_expression(expression: Expression) -> Expression {
    let normalized = normalize_guard_expression_tree(expression);
    match normalized {
        Expression::Boolean(value) => Expression::Boolean(value),
        Expression::Binary(binary) => Expression::Binary(binary),
        other => boolean_compare(other, true),
    }
}

fn normalize_guard_expression_tree(expression: Expression) -> Expression {
    match expression {
        Expression::ArrayLiteral(values) => Expression::ArrayLiteral(Arc::from(
            values
                .iter()
                .cloned()
                .map(normalize_guard_expression_tree)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        )),
        Expression::Binary(binary) => {
            let left = normalize_guard_expression_tree(binary.left);
            let right = normalize_guard_expression_tree(binary.right);
            normalize_binary_expression(BinaryExpression {
                left,
                operator: binary.operator,
                right,
            })
        }
        Expression::Boolean(_)
        | Expression::Call(_)
        | Expression::Cast(_)
        | Expression::Float(_)
        | Expression::Indexed(_)
        | Expression::Integer(_)
        | Expression::Member(_)
        | Expression::Mutable(_)
        | Expression::Name(_)
        | Expression::String(_)
        | Expression::StructLiteral(_) => expression,
    }
}

fn normalize_binary_expression(binary: BinaryExpression) -> Expression {
    if matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) {
        if let Expression::Boolean(flag) = &binary.right {
            return normalize_boolean_condition(
                binary.left,
                positive_branch(binary.operator, *flag),
            );
        }
        if let Expression::Boolean(flag) = &binary.left {
            return normalize_boolean_condition(
                binary.right,
                positive_branch(binary.operator, *flag),
            );
        }
    }

    Expression::Binary(Box::new(binary))
}

fn normalize_boolean_condition(expression: Expression, positive: bool) -> Expression {
    let normalized = normalize_guard_expression_tree(expression);
    match normalized {
        Expression::Binary(binary) => {
            if let Some(operator) = comparison_operator(binary.operator, positive) {
                Expression::Binary(Box::new(BinaryExpression {
                    left: binary.left,
                    operator,
                    right: binary.right,
                }))
            } else if positive {
                Expression::Binary(binary)
            } else {
                boolean_compare(Expression::Binary(binary), false)
            }
        }
        Expression::Boolean(value) => Expression::Boolean(if positive { value } else { !value }),
        other => boolean_compare(other, positive),
    }
}

fn boolean_compare(expression: Expression, expected: bool) -> Expression {
    Expression::Binary(Box::new(BinaryExpression {
        left: expression,
        operator: BinaryOperator::Equal,
        right: Expression::Boolean(expected),
    }))
}

fn positive_branch(operator: BinaryOperator, flag: bool) -> bool {
    match operator {
        BinaryOperator::Equal => flag,
        BinaryOperator::NotEqual => !flag,
        _ => true,
    }
}

fn comparison_operator(operator: BinaryOperator, positive: bool) -> Option<BinaryOperator> {
    let normalized = if positive {
        operator
    } else {
        invert_comparison_operator(operator)?
    };

    Some(normalized)
}

fn invert_comparison_operator(operator: BinaryOperator) -> Option<BinaryOperator> {
    Some(match operator {
        BinaryOperator::Equal => BinaryOperator::NotEqual,
        BinaryOperator::NotEqual => BinaryOperator::Equal,
        BinaryOperator::Greater => BinaryOperator::LessOrEqual,
        BinaryOperator::GreaterOrEqual => BinaryOperator::Less,
        BinaryOperator::Less => BinaryOperator::GreaterOrEqual,
        BinaryOperator::LessOrEqual => BinaryOperator::Greater,
        BinaryOperator::Add
        | BinaryOperator::And
        | BinaryOperator::Divide
        | BinaryOperator::Modulo
        | BinaryOperator::Multiply
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight
        | BinaryOperator::Subtract => return None,
    })
}
