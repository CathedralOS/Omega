//! Syntactic effect and place-root classification used by transparent-result
//! write-frame analysis.
//!
//! The classifier is deliberately conservative: every call is effectful, and
//! malformed expression handles contribute no invented effect. It does not
//! resolve or summarize any call frame.

use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

pub(super) fn expression_is_effectful_for_transparent_result(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    if !expression.is_valid() {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            expression_is_effectful_for_transparent_result(program, dispatch.subject)
                || program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .iter()
                    .any(|arm| {
                        matches!(arm.pattern, typed_trees::expression::MatchPattern::Value(pattern)
                        if expression_is_effectful_for_transparent_result(program, pattern))
                            || expression_is_effectful_for_transparent_result(program, arm.value)
                    })
        }
        ExpressionNode::Atomic(_) | ExpressionNode::Call(_) => true,
        ExpressionNode::Binary(binary) => {
            expression_is_effectful_for_transparent_result(program, binary.left)
                || expression_is_effectful_for_transparent_result(program, binary.right)
        }
        ExpressionNode::Cast(cast) => {
            expression_is_effectful_for_transparent_result(program, cast.value)
        }
        ExpressionNode::Indexed(indexed) => {
            expression_is_effectful_for_transparent_result(program, indexed.collection)
                || expression_is_effectful_for_transparent_result(program, indexed.index)
        }
        ExpressionNode::Member(member) => {
            expression_is_effectful_for_transparent_result(program, member.receiver)
        }
        ExpressionNode::Borrow(inner) => {
            expression_is_effectful_for_transparent_result(program, inner.target)
        }
        ExpressionNode::Unary(unary) => {
            expression_is_effectful_for_transparent_result(program, unary.operand)
        }
        ExpressionNode::ArrayLiteral(elements) => program
            .expression_table
            .expression_handles(*elements)
            .iter()
            .any(|element| expression_is_effectful_for_transparent_result(program, *element)),
        ExpressionNode::Range(range) => {
            expression_is_effectful_for_transparent_result(program, range.start)
                || expression_is_effectful_for_transparent_result(program, range.end)
        }
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| expression_is_effectful_for_transparent_result(program, field.value)),
        typed_trees::literal_expression_pattern!() => false,
    }
}

pub(super) fn frame_place_root_symbol(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => frame_place_root_symbol(program, inner.target),
        ExpressionNode::Indexed(indexed) => frame_place_root_symbol(program, indexed.collection),
        ExpressionNode::Member(member) => frame_place_root_symbol(program, member.receiver),
        ExpressionNode::Name(path) => path.resolved_symbol(),
        _ => None,
    }
}
