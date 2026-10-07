//! Traversal arms identical across the borrow walks. A child-bearing node
//! recurses through `visit`; any other node returns `false` so the caller
//! classifies it under its own rules.

use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::TypedTrees;

/// Recurse `visit` over the child expressions the borrow walks share:
/// match subjects/patterns/values, atomics, array/range/unary operands,
/// binary operands, cast values, and struct-literal fields. Returns
/// `false` for nodes the walks classify differently.
pub(super) fn recurse_shared_children(
    program: &TypedTrees,
    expression: ExpressionHandle,
    visit: &mut dyn FnMut(ExpressionHandle),
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            visit(dispatch.subject);
            for arm in program.expression_table.match_arms(dispatch.arms) {
                if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                    visit(pattern);
                }
                visit(arm.value);
            }
        }
        ExpressionNode::Atomic(atomic) => visit(atomic.value),
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                visit(*value);
            }
        }
        ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                visit(range.start);
            }
            if range.end.is_valid() {
                visit(range.end);
            }
        }
        ExpressionNode::Binary(binary) => {
            visit(binary.left);
            visit(binary.right);
        }
        ExpressionNode::Cast(cast) => visit(cast.value),
        ExpressionNode::Unary(unary) => visit(unary.operand),
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                visit(field.value);
            }
        }
        _ => return false,
    }
    true
}
