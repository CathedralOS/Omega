use typed_trees::expression::{ExpressionHandle, ExpressionNode};

use super::ContractExpressionEvaluator;

impl<'program> ContractExpressionEvaluator<'program, '_> {
    pub(super) fn collection_length(&mut self, expression: ExpressionHandle) -> Option<usize> {
        let resolved = self.resolved_expression(expression).unwrap_or(expression);
        match self.program.expression_table.expression(resolved) {
            ExpressionNode::ArrayLiteral(values) => Some(values.count() as usize),
            ExpressionNode::Borrow(inner) => self.collection_length(inner.target),
            // A fixed array's extent belongs to its declared place, not to its
            // current contents, so a field, local, or parameter keeps that
            // extent through every alias: `&mut subject.control` lends exactly
            // the declared N, and substituting it for a borrowed formal proves
            // `count <= buffer.len` at the call
            // (wiki/spec/terminal-psi/byte_views.md:151-156,
            // structural_access.md:229, counts_and_addresses.md:19-20). The
            // query supplies no extent for slices or bounded byte owners,
            // whose live length their own storage states.
            _ => validation::declared_collection_extent(
                self.program,
                self.caller_machine,
                Some(self.caller_state),
                resolved,
            ),
        }
    }
}
