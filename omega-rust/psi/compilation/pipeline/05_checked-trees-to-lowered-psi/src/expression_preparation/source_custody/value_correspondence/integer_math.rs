//! Mathematical capsules replay authored operators and embeddings. Arena
//! identity alone carries no authority: finite leaves retain their original
//! carrier and selected arithmetic policy through ordinary scalar replay.

use super::{
    BinaryOperator, CheckedIntegerComparisonKind, Context, ExpressionHandle, ExpressionNode,
    PrimitiveType,
};
use checked_trees::{CheckedIntegerMathExpression, CheckedIntegerMathTerm};

#[cfg(test)]
mod tests;

impl Context<'_> {
    pub(super) fn integer_math_comparison(
        &self,
        source: ExpressionHandle,
        kind: CheckedIntegerComparisonKind,
        left: &CheckedIntegerMathExpression,
        right: &CheckedIntegerMathExpression,
        operands: &[(ExpressionHandle, PrimitiveType)],
        depth: usize,
    ) -> bool {
        let ExpressionNode::Binary(binary) = self.checked.expression_table.expression(source)
        else {
            return false;
        };
        let expected = match binary.operator {
            BinaryOperator::Equal | BinaryOperator::NotEqual => CheckedIntegerComparisonKind::Equal,
            BinaryOperator::Less | BinaryOperator::Greater => {
                CheckedIntegerComparisonKind::LessThan
            }
            BinaryOperator::LessOrEqual | BinaryOperator::GreaterOrEqual => {
                CheckedIntegerComparisonKind::LessOrEqual
            }
            _ => return false,
        };
        // NotEqual is represented by the enclosing Boolean Not, whose replay
        // reaches this method through equality rather than directly.
        if kind != expected
            || !self.selected_builtin(source)
            || !(validation::proof_integer_expression(&self.checked.typed, binary.left)
                || validation::proof_integer_expression(&self.checked.typed, binary.right))
        {
            return false;
        }
        let (left_source, right_source) = if matches!(
            binary.operator,
            BinaryOperator::Greater | BinaryOperator::GreaterOrEqual
        ) {
            (binary.right, binary.left)
        } else {
            (binary.left, binary.right)
        };
        self.integer_math(left_source, left, operands, depth)
            && self.integer_math(right_source, right, operands, depth)
    }

    fn integer_math(
        &self,
        source: ExpressionHandle,
        expression: &CheckedIntegerMathExpression,
        operands: &[(ExpressionHandle, PrimitiveType)],
        depth: usize,
    ) -> bool {
        visit_terms(
            self.checked,
            source,
            expression,
            |source, term| match term {
                CheckedIntegerMathTerm::Embed {
                    primitive_type,
                    operand,
                } => {
                    let Some((authored_type, authored_operand)) =
                        validation::integer_embedding_argument(&self.checked.typed, source)
                    else {
                        return false;
                    };
                    *primitive_type == authored_type
                        && operand.primitive_type() == Some(authored_type)
                        && self.scalar(authored_operand, operand, operands, depth + 1)
                }
                CheckedIntegerMathTerm::Literal(literal) => {
                    validation::anonymous_integer_math_literal(&self.checked.typed, source)
                        == Some(*literal)
                }
                CheckedIntegerMathTerm::Add { .. }
                | CheckedIntegerMathTerm::Subtract { .. }
                | CheckedIntegerMathTerm::Multiply { .. } => {
                    let ExpressionNode::Binary(binary) =
                        self.checked.expression_table.expression(source)
                    else {
                        return false;
                    };
                    let expected = match term {
                        CheckedIntegerMathTerm::Add { .. } => BinaryOperator::Add,
                        CheckedIntegerMathTerm::Subtract { .. } => BinaryOperator::Subtract,
                        _ => BinaryOperator::Multiply,
                    };
                    binary.operator == expected
                        && self.selected_builtin(source)
                        && validation::proof_integer_expression(&self.checked.typed, source)
                }
            },
        )
    }
}

/// A receiving owner may impose additional finite-operand metadata checks
/// after full read/value correspondence. Reuse the same source/arena traversal
/// to find each authored embedding, rather than treating an arena position as
/// an authored expression or dropping the finite operand's landing policy.
pub(crate) fn validate_embedding_sources(
    checked: &super::CheckedTrees,
    source: ExpressionHandle,
    expression: &CheckedIntegerMathExpression,
    finite: impl Fn(ExpressionHandle, &super::Scalar) -> bool,
) -> bool {
    visit_terms(checked, source, expression, |source, term| match term {
        CheckedIntegerMathTerm::Embed {
            primitive_type,
            operand,
        } => validation::integer_embedding_argument(&checked.typed, source).is_some_and(
            |(primitive, authored)| primitive == *primitive_type && finite(authored, operand),
        ),
        _ => true,
    })
}

fn visit_terms(
    checked: &super::CheckedTrees,
    source: ExpressionHandle,
    expression: &CheckedIntegerMathExpression,
    visit: impl Fn(ExpressionHandle, &CheckedIntegerMathTerm) -> bool,
) -> bool {
    let mut pending = vec![(source, expression.root)];
    // Source correspondence visits occurrences, not just unique arena rows.
    // A source-sized budget rejects forged cycles and exponential DAG expansion
    // without recursive host-stack traversal or trusting capsule dimensions.
    let mut remaining = checked
        .expression_table
        .expression_count()
        .saturating_mul(4);
    while let Some((source, handle)) = pending.pop() {
        if remaining == 0
            || !expression.terms.is_valid(handle)
            || !checked.expression_table.expression_is_valid(source)
        {
            return false;
        }
        remaining -= 1;
        let term = expression.terms.get(handle);
        if !visit(source, term) {
            return false;
        }
        match term {
            CheckedIntegerMathTerm::Add { left, right }
            | CheckedIntegerMathTerm::Subtract { left, right }
            | CheckedIntegerMathTerm::Multiply { left, right } => {
                let ExpressionNode::Binary(binary) = checked.expression_table.expression(source)
                else {
                    return false;
                };
                pending.push((binary.right, *right));
                pending.push((binary.left, *left));
            }
            CheckedIntegerMathTerm::Embed { .. } | CheckedIntegerMathTerm::Literal(_) => {}
        }
    }
    true
}
