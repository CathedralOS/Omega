use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};

use super::ContractExpressionEvaluator;

impl ContractExpressionEvaluator<'_, '_> {
    pub(super) fn collection_length(&self, expression: ExpressionHandle) -> Option<usize> {
        let expression = self.resolved_expression(expression).unwrap_or(expression);
        match self.program.expression_table.expression(expression) {
            ExpressionNode::ArrayLiteral(values) => Some(values.count() as usize),
            ExpressionNode::Mutable(inner) => self.collection_length(*inner),
            _ => None,
        }
    }
}
