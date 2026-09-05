//! Syntactic effect and place-root classification used by transparent-result
//! write-frame analysis.
//!
//! The classifier is deliberately conservative: only compiler-owned slice
//! views are effect-free calls, and malformed expression handles contribute no
//! invented effect. It does not resolve or summarize any call frame.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};

pub(super) fn expression_is_effectful_for_transparent_result(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> bool {
    if !expression.is_valid() {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(_) => true,
        ExpressionNode::Call(call) => {
            !call_is_effect_free_slice_view(program, call)
                || expression_is_effectful_for_transparent_result(program, call.receiver)
        }
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
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}

pub(super) fn call_is_transparent_mutable_slice_view(
    program: &TypedTrees,
    call: &TableCallExpression,
) -> bool {
    call.target.as_str() == "as_mut_slice" && call_is_effect_free_slice_view(program, call)
}

fn call_is_effect_free_slice_view(program: &TypedTrees, call: &TableCallExpression) -> bool {
    matches!(call.target.as_str(), "as_slice" | "as_mut_slice")
        && call.receiver.is_valid()
        && program
            .expression_table
            .expression_handles(call.arguments)
            .is_empty()
}

pub(super) fn frame_place_root_symbol(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => frame_place_root_symbol(program, inner.target),
        ExpressionNode::Indexed(indexed) => frame_place_root_symbol(program, indexed.collection),
        ExpressionNode::Member(member) => frame_place_root_symbol(program, member.receiver),
        ExpressionNode::Name(path) => path
            .head_symbol
            .is_valid()
            .then_some(path.head_symbol)
            .or_else(|| path.symbol.is_valid().then_some(path.symbol)),
        _ => None,
    }
}
