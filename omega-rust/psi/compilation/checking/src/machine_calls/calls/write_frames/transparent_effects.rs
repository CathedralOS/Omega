//! Syntactic effect and place-root classification used by transparent-result
//! write-frame analysis.
//!
//! The classifier is deliberately conservative: every call is effectful, and
//! malformed expression handles contribute no invented effect. It does not
//! resolve or summarize any call frame.

use crate::frozen_program::expression_table;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

pub(super) fn expression_is_effectful_for_transparent_result(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    program.subtree_any(expression, typed_trees::SubtreeEdges::VALUES, |_, node| {
        if matches!(node, ExpressionNode::Atomic(_) | ExpressionNode::Call(_)) {
            typed_trees::SubtreeVisit::Found
        } else {
            typed_trees::SubtreeVisit::Descend
        }
    })
}

pub(super) fn frame_place_root_symbol(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    match expression_table(program).expression(expression) {
        ExpressionNode::Borrow(inner) => frame_place_root_symbol(program, inner.target),
        ExpressionNode::Indexed(indexed) => frame_place_root_symbol(program, indexed.collection),
        ExpressionNode::Member(member) => frame_place_root_symbol(program, member.receiver),
        ExpressionNode::Name(path) => path.resolved_symbol(),
        _ => None,
    }
}
