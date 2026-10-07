//! Plain structural children of an expression: the descent every
//! worklist walk shares once its own semantic arms are handled.

use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

/// Pushes the expression's ordinary children onto `pending`. Covers the
/// node kinds with no walk-specific semantics; callers match their own
/// significant kinds first and delegate the rest here.
pub(crate) fn push_expression_children(
    program: &TypedTrees,
    expression: ExpressionHandle,
    pending: &mut Vec<ExpressionHandle>,
) {
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            pending.push(binary.left);
            pending.push(binary.right);
        }
        ExpressionNode::Unary(unary) => pending.push(unary.operand),
        ExpressionNode::Cast(cast) => pending.push(cast.value),
        ExpressionNode::Indexed(indexed) => {
            pending.push(indexed.collection);
            pending.push(indexed.index);
        }
        ExpressionNode::Member(member) => pending.push(member.receiver),
        ExpressionNode::Range(range) => {
            pending.push(range.start);
            pending.push(range.end);
        }
        ExpressionNode::ArrayLiteral(values) => {
            pending.extend(
                program
                    .expression_table
                    .expression_handles(*values)
                    .iter()
                    .copied(),
            );
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                pending.push(field.value);
            }
        }
        _ => {}
    }
}
