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

/// Every direct child handle of the expression — the complete grammar walk
/// for traversals that follow all owned subtrees, not one lane's scalar
/// subset.
pub(crate) fn push_every_expression_child(
    program: &TypedTrees,
    node: &ExpressionNode,
    pending: &mut Vec<ExpressionHandle>,
) {
    match node {
        ExpressionNode::Match(dispatch) => {
            pending.push(dispatch.subject);
            for arm in program.expression_table.match_arms(dispatch.arms) {
                if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                    pending.push(pattern);
                }
                pending.push(arm.value);
            }
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
        ExpressionNode::Atomic(atomic) => {
            pending.push(atomic.value);
            pending.push(atomic.result);
        }
        ExpressionNode::Binary(binary) => {
            pending.push(binary.left);
            pending.push(binary.right);
        }
        ExpressionNode::Call(call) => {
            pending.push(call.receiver);
            pending.extend(
                program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .copied(),
            );
        }
        ExpressionNode::Cast(cast) => pending.push(cast.value),
        ExpressionNode::Indexed(indexed) => {
            pending.push(indexed.collection);
            pending.push(indexed.index);
        }
        ExpressionNode::Member(member) => pending.push(member.receiver),
        ExpressionNode::Borrow(borrow) => pending.push(borrow.target),
        ExpressionNode::Range(range) => {
            pending.push(range.start);
            pending.push(range.end);
        }
        ExpressionNode::StructLiteral(literal) => {
            pending.extend(
                program
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| field.value),
            );
        }
        ExpressionNode::Unary(unary) => pending.push(unary.operand),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}
