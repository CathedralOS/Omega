use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

use super::ContractExpressionEvaluator;

impl ContractExpressionEvaluator<'_, '_> {
    pub(super) fn integer_value(&self, expression: ExpressionHandle) -> Option<i64> {
        // Re-entering an expression whose evaluation is still in progress
        // means call-site following looped (recursive machine); stand down
        // with unknown rather than overflow the stack.
        Self::guarding_cycles(&self.active_evaluations, expression, || {
            self.integer_value_of_resolved(expression)
        })
    }

    fn integer_value_of_resolved(&self, expression: ExpressionHandle) -> Option<i64> {
        let expression = self.resolved_expression(expression).unwrap_or(expression);
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Binary(binary) => {
                let left = self.integer_value(binary.left)?;
                let right = self.integer_value(binary.right)?;
                match binary.operator {
                    BinaryOperator::Add => left.checked_add(right),
                    BinaryOperator::Divide => {
                        (right != 0).then(|| left.checked_div(right)).flatten()
                    }
                    BinaryOperator::Modulo => {
                        (right != 0).then(|| left.checked_rem(right)).flatten()
                    }
                    BinaryOperator::Multiply => left.checked_mul(right),
                    BinaryOperator::Subtract => left.checked_sub(right),
                    BinaryOperator::And
                    | BinaryOperator::Equal
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Or
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight => None,
                }
            }
            ExpressionNode::Integer(value) => Some(*value),
            ExpressionNode::Member(member) if member.member.as_str() == "len" => self
                .collection_length(member.receiver)
                .and_then(|length| i64::try_from(length).ok()),
            ExpressionNode::Mutable(inner) => self.integer_value(*inner),
            _ => None,
        }
    }
}
