//! Structural expression edges and occurrence collection shared by semantic checks.
//!
//! `children` exposes every immediate value edge in source order. It retains
//! invalid handles and repeated edges: destination custody uses their exact
//! positions, including an absent call receiver at position zero. Consumers own
//! pruning, live-handle validation, and visitation state.
//!
//! `collect_expression_nodes` is the recursive preorder used for occurrence
//! queries. It ignores zero handles and records each generational handle once.
//! This is structural traversal; it does not select executable match arms or
//! propagate destination types.

use std::collections::HashSet;
use typed_trees::TypedTrees;
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, MatchPattern, TableMatchExpression,
};

pub(crate) fn children(
    program: &TypedTrees,
    node: &ExpressionNode,
    mut child: impl FnMut(ExpressionHandle),
) {
    match node {
        ExpressionNode::Match(dispatch) => {
            for expression in match_children(program, *dispatch) {
                child(expression);
            }
        }
        ExpressionNode::Binary(binary) => {
            child(binary.left);
            child(binary.right);
        }
        ExpressionNode::Unary(unary) => child(unary.operand),
        ExpressionNode::Borrow(borrow) => child(borrow.target),
        ExpressionNode::Cast(cast) => child(cast.value),
        ExpressionNode::Atomic(atomic) => {
            child(atomic.value);
            child(atomic.result);
        }
        ExpressionNode::ArrayLiteral(elements) => {
            for element in program.expression_table.expression_handles(*elements) {
                child(*element);
            }
        }
        ExpressionNode::Call(call) => {
            child(call.receiver);
            for argument in program.expression_table.expression_handles(call.arguments) {
                child(*argument);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            child(indexed.collection);
            child(indexed.index);
        }
        ExpressionNode::Member(member) => child(member.receiver),
        ExpressionNode::Range(range) => {
            child(range.start);
            child(range.end);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                child(field.value);
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

/// Structural readers visit every source edge, not just executable arms.
pub(crate) fn match_children(
    program: &TypedTrees,
    dispatch: TableMatchExpression,
) -> impl Iterator<Item = ExpressionHandle> + '_ {
    std::iter::once(dispatch.subject).chain(
        program
            .expression_table
            .match_arms(dispatch.arms)
            .iter()
            .flat_map(|arm| {
                let pattern = match arm.pattern {
                    MatchPattern::Value(pattern) => Some(pattern),
                    MatchPattern::Wildcard => None,
                };
                pattern.into_iter().chain(std::iter::once(arm.value))
            }),
    )
}

/// Append first visits in recursive preorder, sharing the caller's occurrence roster.
pub(crate) fn collect_expression_nodes(
    program: &TypedTrees,
    expression: ExpressionHandle,
    nodes: &mut Vec<ExpressionHandle>,
    seen: &mut HashSet<ExpressionHandle>,
) {
    if !expression.is_valid() || !seen.insert(expression) {
        return;
    }
    nodes.push(expression);
    children(
        program,
        program.expression_table.expression(expression),
        |child| {
            collect_expression_nodes(program, child, nodes, seen);
        },
    );
}
