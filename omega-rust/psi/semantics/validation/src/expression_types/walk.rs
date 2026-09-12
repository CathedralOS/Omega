//! Shared expression traversal for occurrence-scoped semantic queries.
//! Keep one visited roster so shared children and malformed cycles cannot make
//! owner lookup duplicate or indefinitely revisit an expression occurrence.

use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

pub(crate) fn collect_expression_nodes(
    program: &TypedTrees,
    expression: ExpressionHandle,
    nodes: &mut Vec<ExpressionHandle>,
) {
    if !expression.is_valid() || nodes.contains(&expression) {
        return;
    }
    nodes.push(expression);
    let mut recurse = |child| collect_expression_nodes(program, child, nodes);
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            for child in super::match_children(program, *dispatch) {
                recurse(child);
            }
        }
        ExpressionNode::Atomic(atomic) => {
            recurse(atomic.value);
            recurse(atomic.result);
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in program.expression_table.expression_handles(*elements) {
                recurse(*element);
            }
        }
        ExpressionNode::Binary(binary) => {
            recurse(binary.left);
            recurse(binary.right);
        }
        ExpressionNode::Borrow(inner) => recurse(inner.target),
        ExpressionNode::Call(call) => {
            recurse(call.receiver);
            for argument in program.expression_table.expression_handles(call.arguments) {
                recurse(*argument);
            }
        }
        ExpressionNode::Cast(cast) => recurse(cast.value),
        ExpressionNode::Indexed(indexed) => {
            recurse(indexed.collection);
            recurse(indexed.index);
        }
        ExpressionNode::Member(member) => recurse(member.receiver),
        ExpressionNode::Range(range) => {
            recurse(range.start);
            recurse(range.end);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                recurse(field.value);
            }
        }
        ExpressionNode::Unary(unary) => recurse(unary.operand),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}
