//! Assignment-target shape queries for write-frame inference.
//!
//! This leaf recovers the declared target type and classifies the structural
//! place shapes used by transparent-result analysis. It does not infer aliases
//! or resolve call frames.

use super::transparent_effects::expression_is_effectful_for_transparent_result;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

pub(super) fn expression_is_effectful_indexed_place(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            expression_is_effectful_indexed_place(program, inner.target)
        }
        ExpressionNode::Member(member) => {
            expression_is_effectful_indexed_place(program, member.receiver)
        }
        ExpressionNode::Indexed(indexed)
            if expression_is_effectful_for_transparent_result(program, indexed.index) =>
        {
            true
        }
        _ => false,
    }
}
