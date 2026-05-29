use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

use super::ContractExpressionEvaluator;

impl ContractExpressionEvaluator<'_, '_> {
    pub(super) fn boolean_value(&self, expression: ExpressionHandle) -> Option<bool> {
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Boolean(value) => Some(*value),
            ExpressionNode::Binary(binary) => match binary.operator {
                BinaryOperator::And => {
                    Some(self.boolean_value(binary.left)? && self.boolean_value(binary.right)?)
                }
                BinaryOperator::Or => {
                    Some(self.boolean_value(binary.left)? || self.boolean_value(binary.right)?)
                }
                BinaryOperator::Equal => {
                    Some(self.integer_value(binary.left)? == self.integer_value(binary.right)?)
                }
                BinaryOperator::Greater => {
                    Some(self.integer_value(binary.left)? > self.integer_value(binary.right)?)
                }
                BinaryOperator::GreaterOrEqual => {
                    Some(self.integer_value(binary.left)? >= self.integer_value(binary.right)?)
                }
                BinaryOperator::Less => {
                    Some(self.integer_value(binary.left)? < self.integer_value(binary.right)?)
                }
                BinaryOperator::LessOrEqual => {
                    Some(self.integer_value(binary.left)? <= self.integer_value(binary.right)?)
                }
                BinaryOperator::NotEqual => {
                    Some(self.integer_value(binary.left)? != self.integer_value(binary.right)?)
                }
                BinaryOperator::Add
                | BinaryOperator::Divide
                | BinaryOperator::Modulo
                | BinaryOperator::Multiply
                | BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight
                | BinaryOperator::Subtract => None,
            },
            ExpressionNode::Mutable(inner) => self.boolean_value(*inner),
            _ => None,
        }
    }
}
