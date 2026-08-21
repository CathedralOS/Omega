//! Pure local-alias queries for caller-visible write frames.
//!
//! This leaf rebases relative paths through already-canonical alias origins
//! and detects syntactic mutable reborrows of known local aliases. It neither
//! infers origins nor mutates alias bindings.

use super::place_paths::{
    FramePathPrecision, FramePlaceOrigin, append_place_suffix, split_place_root,
};
use crate::arithmetic_domains;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};

pub(super) fn rebase_local_alias_path(
    relative: &str,
    aliases: &[(String, FramePlaceOrigin)],
) -> String {
    let (root, suffix) = split_place_root(relative);
    aliases
        .iter()
        .find_map(|(alias, origin)| {
            (alias == root).then(|| match origin.precision {
                FramePathPrecision::Exact => append_place_suffix(&origin.path, suffix),
                FramePathPrecision::CollectionCoarse => origin.path.clone(),
            })
        })
        .unwrap_or_else(|| relative.to_owned())
}

pub(super) fn expression_reborrows_local_alias_binding(
    program: &TypedTrees,
    expression: ExpressionHandle,
    aliases: &[(String, FramePlaceOrigin)],
) -> bool {
    if !expression.is_valid() {
        return false;
    }
    let visit = |child| expression_reborrows_local_alias_binding(program, child, aliases);
    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => {
            let borrows_binding = matches!(
                program.expression_table.expression(*inner),
                ExpressionNode::Name(_)
            ) && arithmetic_domains::place_path(program, *inner)
                .is_some_and(|path| aliases.iter().any(|(alias, _)| path == *alias));
            borrows_binding || visit(*inner)
        }
        ExpressionNode::Atomic(atomic) => visit(atomic.value) || visit(atomic.result),
        ExpressionNode::Call(call) => {
            (call.receiver.is_valid() && visit(call.receiver))
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| visit(*argument))
        }
        ExpressionNode::Binary(binary) => visit(binary.left) || visit(binary.right),
        ExpressionNode::Unary(unary) => visit(unary.operand),
        ExpressionNode::Cast(cast) => visit(cast.value),
        ExpressionNode::Indexed(indexed) => visit(indexed.collection) || visit(indexed.index),
        ExpressionNode::Member(member) => visit(member.receiver),
        ExpressionNode::ArrayLiteral(elements) => program
            .expression_table
            .expression_handles(*elements)
            .iter()
            .any(|element| visit(*element)),
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| visit(field.value)),
        ExpressionNode::Range(range) => visit(range.start) || visit(range.end),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}
