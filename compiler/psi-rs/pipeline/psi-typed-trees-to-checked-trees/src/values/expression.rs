use super::*;
use psi_typed_trees::expression::ExpressionNode;

impl ValueFactBuilder<'_, '_> {
    pub(super) fn collect_expression_children(&mut self, expression: ExpressionHandle) {
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Atomic(atomic) => {
                self.collect_nested_expression(expression, atomic.value)
            }
            ExpressionNode::ArrayLiteral(values) => {
                for value in self
                    .program
                    .expression_table
                    .expression_handles(*values)
                    .iter()
                    .copied()
                {
                    self.collect_nested_expression(expression, value);
                }
            }
            ExpressionNode::Binary(binary) => {
                self.collect_nested_expression(expression, binary.left);
                self.collect_nested_expression(expression, binary.right);
            }
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
            ExpressionNode::Cast(cast) => {
                self.collect_nested_expression(expression, cast.value);
            }
            ExpressionNode::Call(call) => {
                if call.receiver.is_valid() {
                    self.collect_nested_expression(expression, call.receiver);
                }
                for argument in self
                    .program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .copied()
                {
                    self.collect_nested_expression(expression, argument);
                }
            }
            ExpressionNode::Indexed(indexed) => {
                self.collect_nested_expression(expression, indexed.collection);
                self.collect_nested_expression(expression, indexed.index);
            }
            ExpressionNode::Member(member) => {
                self.collect_nested_expression(expression, member.receiver);
            }
            ExpressionNode::Mutable(value) => {
                self.collect_nested_expression(expression, *value);
            }
            ExpressionNode::Unary(unary) => {
                self.collect_nested_expression(expression, unary.operand);
            }
            ExpressionNode::Range(range) => {
                self.collect_nested_expression(expression, range.start);
                self.collect_nested_expression(expression, range.end);
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in self.program.expression_table.struct_fields(literal.fields) {
                    self.collect_nested_expression(expression, field.value);
                }
            }
        }
    }
}
