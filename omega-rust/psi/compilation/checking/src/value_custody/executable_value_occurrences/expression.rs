use super::ExpressionHandle;
use super::ValueOccurrenceVisitor;

impl ValueOccurrenceVisitor<'_, '_> {
    pub(super) fn collect_expression_children(&mut self, expression: ExpressionHandle) {
        let program = self.program;
        crate::expression_traversal::executable_children(
            program,
            program.expression_table.expression(expression),
            |child| self.collect_nested_expression(expression, child),
        );
    }
}
