use psi_checked_trees::expression::{
    BinaryExpression, BinaryOperator, Expression, ExpressionHandle, ExpressionNode,
    ExpressionTable, TableBinaryExpression,
};
pub(super) fn normalize_guard_expression_into_table(
    expression: Expression,
    output: &mut ExpressionTable,
) -> ExpressionHandle {
    let normalized = normalize_guard_expression_tree_into_table(expression, output);
    match output.expression(normalized) {
        ExpressionNode::Boolean(_) | ExpressionNode::Binary(_) => normalized,
        _ => boolean_compare_into_table(output, normalized, true),
    }
}

fn normalize_guard_expression_tree_into_table(
    expression: Expression,
    output: &mut ExpressionTable,
) -> ExpressionHandle {
    match expression {
        Expression::ArrayLiteral(values) => {
            let values_span = output.reserve_expression_handles(
                values
                    .len()
                    .try_into()
                    .expect("array literal expression span count overflow"),
            );
            for (offset, value) in values.iter().cloned().enumerate() {
                let value = normalize_guard_expression_tree_into_table(value, output);
                output.set_expression_handle_at_offset(
                    values_span,
                    offset
                        .try_into()
                        .expect("array literal expression span count overflow"),
                    value,
                );
            }
            output.insert(ExpressionNode::ArrayLiteral(values_span))
        }
        Expression::Binary(binary) => normalize_binary_expression_into_table(*binary, output),
        other => output.insert_tree(&other),
    }
}

fn normalize_binary_expression_into_table(
    binary: BinaryExpression,
    output: &mut ExpressionTable,
) -> ExpressionHandle {
    if matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) {
        if let Expression::Boolean(flag) = &binary.right {
            return normalize_boolean_condition_into_table(
                binary.left,
                positive_branch(binary.operator, *flag),
                output,
            );
        }
        if let Expression::Boolean(flag) = &binary.left {
            return normalize_boolean_condition_into_table(
                binary.right,
                positive_branch(binary.operator, *flag),
                output,
            );
        }
    }

    let left = normalize_guard_expression_tree_into_table(binary.left, output);
    let right = normalize_guard_expression_tree_into_table(binary.right, output);
    output.insert(ExpressionNode::Binary(TableBinaryExpression {
        left,
        operator: binary.operator,
        right,
    }))
}

fn normalize_boolean_condition_into_table(
    expression: Expression,
    positive: bool,
    output: &mut ExpressionTable,
) -> ExpressionHandle {
    let normalized = normalize_guard_expression_tree_into_table(expression, output);
    match output.expression(normalized) {
        ExpressionNode::Binary(binary) => {
            let binary = *binary;
            if let Some(operator) = comparison_operator(binary.operator, positive) {
                output.insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: binary.left,
                    operator,
                    right: binary.right,
                }))
            } else if positive {
                normalized
            } else {
                boolean_compare_into_table(output, normalized, false)
            }
        }
        ExpressionNode::Boolean(value) => output.insert(ExpressionNode::Boolean(if positive {
            *value
        } else {
            !*value
        })),
        _ => boolean_compare_into_table(output, normalized, positive),
    }
}

fn boolean_compare_into_table(
    output: &mut ExpressionTable,
    expression: ExpressionHandle,
    expected: bool,
) -> ExpressionHandle {
    let right = output.insert(ExpressionNode::Boolean(expected));
    output.insert(ExpressionNode::Binary(TableBinaryExpression {
        left: expression,
        operator: BinaryOperator::Equal,
        right,
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
        | BinaryOperator::BitwiseAnd
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor
        | BinaryOperator::Divide
        | BinaryOperator::Modulo
        | BinaryOperator::Multiply
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight
        | BinaryOperator::Subtract => return None,
    })
}
