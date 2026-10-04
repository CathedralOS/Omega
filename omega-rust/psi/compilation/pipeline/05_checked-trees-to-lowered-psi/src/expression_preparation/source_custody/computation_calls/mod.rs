//! Rejoin computed invocations to captured expression occurrences, not spans.
//! Semantic casts are part of that ordering: equal final tags cannot justify
//! skipping an explicit erasure followed by reintroduction. Only casts whose
//! operand is independently known to be bare may be peeled as payload wrappers.
use super::{
    CheckedTrees, ExpressionHandle, ExpressionNode, LoweringError, preceding_local,
    structural_parameter_at, unsupported,
};

use crate::expression_preparation::source_custody::borrow_occurrences as borrow_rows;
mod borrowed_nominal_arguments;
mod borrowed_storage;
pub(crate) mod byte_comparisons;
pub(crate) mod byte_predicates;
mod dispatch;
mod mixed_arguments;
mod operand_scopes;
mod owned_arguments;
pub(crate) mod primitive_arguments;
mod qualifications;
mod view_arguments;

pub(crate) use mixed_arguments::access_occurrences::{
    rejoin as rejoin_call_accesses, replay as replay_call_accesses,
};

// A retained computation cannot make a not-yet-established source local
// available. This checks declaration order, not pure-expression semantics.
// Earlier mutable locals remain available to the storage-read evaluator.

// This is expression-tree membership only. Call ordinals remain the retained
// semantic traversal's identity, and unselected syntax need not have a flow row.
pub(crate) fn authored_expressions(
    checked: &CheckedTrees,
    root: ExpressionHandle,
) -> Result<Vec<ExpressionHandle>, LoweringError> {
    expression_membership(checked, root, false)
}

// Availability still uses the complete authored tree. Only call coverage and
// executable operand custody exclude independently proved dead Match arms.
fn expression_membership(
    checked: &CheckedTrees,
    root: ExpressionHandle,
    executable_only: bool,
) -> Result<Vec<ExpressionHandle>, LoweringError> {
    let table = &checked.expression_table;
    let mut expressions = Vec::new();
    let mut active = Vec::new();
    let mut pending = vec![(root, false)];
    while let Some((expression, exiting)) = pending.pop() {
        if exiting {
            active.pop();
            expressions.push(expression);
            continue;
        }
        if !table.expression_is_valid(expression) {
            return unsupported("computed invocation has a stale authored expression");
        }
        if active.contains(&expression) {
            return unsupported("computed invocation has a cyclic authored expression");
        }
        if expressions.contains(&expression) {
            continue;
        }
        active.push(expression);
        pending.push((expression, true));
        let mut children = Vec::new();
        match table.expression(expression) {
            ExpressionNode::Atomic(atomic) => {
                children.push(atomic.value);
                if atomic.result.is_valid() {
                    children.push(atomic.result);
                }
            }
            ExpressionNode::ArrayLiteral(elements) => {
                children.extend_from_slice(table.expression_handles(*elements))
            }
            ExpressionNode::Binary(binary) => children.extend([binary.left, binary.right]),
            ExpressionNode::Match(dispatch) => {
                if executable_only
                    && let Some(selected) =
                        operand_scopes::anonymous_match_value(checked, expression)
                {
                    pending.push((selected, false));
                    continue;
                }
                children.push(dispatch.subject);
                let arms = table.match_arms(dispatch.arms);
                if arms.len() != dispatch.arms.len() {
                    return unsupported("computed dispatch has a stale authored arm span");
                }
                for arm in arms {
                    if let checked_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                        children.push(pattern);
                    }
                    children.push(arm.value);
                }
            }
            ExpressionNode::Borrow(borrow) => children.push(borrow.target),
            ExpressionNode::Call(call) => {
                if call.receiver.is_valid() {
                    children.push(call.receiver);
                }
                children.extend_from_slice(table.expression_handles(call.arguments));
            }
            ExpressionNode::Cast(cast) => children.push(cast.value),
            ExpressionNode::Indexed(indexed) => {
                children.extend([indexed.collection, indexed.index])
            }
            ExpressionNode::Member(member) => children.push(member.receiver),
            ExpressionNode::Range(range) => {
                if range.start.is_valid() {
                    children.push(range.start);
                }
                if range.end.is_valid() {
                    children.push(range.end);
                }
            }
            ExpressionNode::StructLiteral(literal) => children.extend(
                table
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| field.value),
            ),
            ExpressionNode::Unary(unary) => children.push(unary.operand),
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
        pending.extend(children.into_iter().rev().map(|child| (child, false)));
    }
    Ok(expressions)
}
