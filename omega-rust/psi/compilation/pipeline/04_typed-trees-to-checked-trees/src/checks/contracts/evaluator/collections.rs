use typed_trees::expression::{ExpressionHandle, ExpressionNode};

use super::ContractExpressionEvaluator;

impl<'program> ContractExpressionEvaluator<'program, '_> {
    pub(super) fn collection_length(&mut self, expression: ExpressionHandle) -> Option<usize> {
        let resolved = self.resolved_expression(expression).unwrap_or(expression);
        match self.program.expression_table.expression(resolved) {
            ExpressionNode::ArrayLiteral(values) => Some(values.count() as usize),
            ExpressionNode::Borrow(inner) => self.collection_length(inner.target),
            _ => None,
        }
    }
}
