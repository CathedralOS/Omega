use psi_typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
};

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
                    if let (Some(left), Some(right)) = (
                        self.integer_value(binary.left),
                        self.integer_value(binary.right),
                    ) {
                        Some(left == right)
                    } else {
                        let (left_min, left_max) = self.integer_bounds(binary.left)?;
                        let (right_min, right_max) = self.integer_bounds(binary.right)?;
                        (left_min == left_max && right_min == right_max && left_min == right_min)
                            .then_some(true)
                    }
                }
                BinaryOperator::Greater => {
                    if let (Some(left), Some(right)) = (
                        self.integer_value(binary.left),
                        self.integer_value(binary.right),
                    ) {
                        Some(left > right)
                    } else {
                        let (left_min, _) = self.integer_bounds(binary.left)?;
                        let (_, right_max) = self.integer_bounds(binary.right)?;
                        (left_min > right_max).then_some(true)
                    }
                }
                BinaryOperator::GreaterOrEqual => {
                    if let (Some(left), Some(right)) = (
                        self.integer_value(binary.left),
                        self.integer_value(binary.right),
                    ) {
                        Some(left >= right)
                    } else {
                        let (left_min, _) = self.integer_bounds(binary.left)?;
                        let (_, right_max) = self.integer_bounds(binary.right)?;
                        (left_min >= right_max).then_some(true)
                    }
                }
                BinaryOperator::Less => {
                    if let (Some(left), Some(right)) = (
                        self.integer_value(binary.left),
                        self.integer_value(binary.right),
                    ) {
                        Some(left < right)
                    } else {
                        let (_, left_max) = self.integer_bounds(binary.left)?;
                        let (right_min, _) = self.integer_bounds(binary.right)?;
                        (left_max < right_min).then_some(true)
                    }
                }
                BinaryOperator::LessOrEqual => {
                    if let (Some(left), Some(right)) = (
                        self.integer_value(binary.left),
                        self.integer_value(binary.right),
                    ) {
                        Some(left <= right)
                    } else {
                        let (_, left_max) = self.integer_bounds(binary.left)?;
                        let (right_min, _) = self.integer_bounds(binary.right)?;
                        (left_max <= right_min).then_some(true)
                    }
                }
                BinaryOperator::NotEqual => {
                    if let (Some(left), Some(right)) = (
                        self.integer_value(binary.left),
                        self.integer_value(binary.right),
                    ) {
                        Some(left != right)
                    } else {
                        let (left_min, left_max) = self.integer_bounds(binary.left)?;
                        let (right_min, right_max) = self.integer_bounds(binary.right)?;
                        (left_max < right_min || right_max < left_min).then_some(true)
                    }
                }
                BinaryOperator::Add
                | BinaryOperator::BitwiseAnd
                | BinaryOperator::BitwiseOr
                | BinaryOperator::BitwiseXor
                | BinaryOperator::Divide
                | BinaryOperator::Modulo
                | BinaryOperator::Multiply
                | BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight
                | BinaryOperator::Subtract => None,
            },
            ExpressionNode::Name(_) => {
                let resolved = self.resolved_expression(expression)?;
                (resolved != expression)
                    .then(|| self.boolean_value(resolved))
                    .flatten()
            }
            ExpressionNode::Mutable(inner) => self.boolean_value(*inner),
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                Some(!self.boolean_value(unary.operand)?)
            }
            _ => None,
        }
    }
}
