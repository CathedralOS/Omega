//! Parameter-relative alias identity and reborrow queries.
//!
//! This leaf owns the narrow origin carrier shared by transparent-result
//! analysis, plus syntax-only binding lookup and mutable-reborrow detection. It
//! does not infer a call result or resolve a frame.

use super::place_paths::{FramePlaceOrigin, frame_place_path, split_place_root};
use super::transparent_effects::frame_place_root_symbol;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;
use typed_trees::types::TypeReferenceNode;

#[derive(Debug, Clone)]
pub(super) struct ParameterRelativeFrameOrigin {
    pub(super) place: FramePlaceOrigin,
    /// Zero means a proven caller-isolated local with no caller parameter.
    /// Such origins may compose inside a helper but cannot be exported as its
    /// returned-place relation.
    pub(super) parameter_symbol: SymbolHandle,
}

pub(super) fn expression_reborrows_transparent_alias_binding(
    program: &TypedTrees,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
) -> bool {
    if !expression.is_valid() {
        return false;
    }
    let visit =
        |child| expression_reborrows_transparent_alias_binding(program, child, parameters, aliases);
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            let reborrows_binding = inner.access.is_exclusive()
                && matches!(
                    program.expression_table.expression(inner.target),
                    ExpressionNode::Name(_)
                )
                && frame_place_path(program, inner.target).is_some_and(|place| {
                    let (root, suffix) = split_place_root(&place.path);
                    if !suffix.is_empty() {
                        return false;
                    }
                    let root_symbol = frame_place_root_symbol(program, inner.target);
                    parameters.iter().any(|parameter| {
                        matches!(
                            program
                                .type_reference_table
                                .type_reference(parameter.type_reference),
                            TypeReferenceNode::Reference { access, .. }
                                if access.is_exclusive()
                        ) && (root_symbol == Some(parameter.symbol)
                            || parameter.is_self && root == "self"
                            || root == parameter.name.as_str())
                    }) || aliases.iter().any(|(name, symbol, _)| {
                        root_symbol.is_some_and(|root| {
                            root.is_valid() && symbol.is_valid() && root == *symbol
                        }) || root == name
                    })
                });
            reborrows_binding || visit(inner.target)
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

pub(super) fn parameter_relative_alias_position(
    program: &TypedTrees,
    expression: ExpressionHandle,
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
) -> Option<usize> {
    let place = frame_place_path(program, expression)?;
    let (root, suffix) = split_place_root(&place.path);
    if !suffix.is_empty() {
        return None;
    }
    let root_symbol = frame_place_root_symbol(program, expression);
    aliases.iter().position(|(name, symbol, _)| {
        let exact_symbol =
            root_symbol.is_some_and(|root| root.is_valid() && symbol.is_valid() && root == *symbol);
        let unresolved_name = root_symbol.is_none_or(|root| !root.is_valid()) && name == root;
        exact_symbol || unresolved_name
    })
}
