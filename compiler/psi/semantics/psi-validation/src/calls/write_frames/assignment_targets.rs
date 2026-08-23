//! Assignment-target shape queries for write-frame inference.
//!
//! This leaf recovers the declared target type and classifies the structural
//! place shapes used by transparent-result analysis. It does not infer aliases
//! or resolve call frames.

use super::transparent_effects::expression_is_effectful_for_transparent_result;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;
use psi_typed_trees::types::TypeReferenceHandle;

pub(super) fn assignment_target_type(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    target: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    crate::places::declared_place_type(program, machine, Some(state), target).or_else(|| {
        crate::places::declared_indexed_projection_type(program, machine, Some(state), target)
    })
}

pub(super) fn expression_is_effectful_indexed_place(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => expression_is_effectful_indexed_place(program, *inner),
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

/// Effects are permitted only along the place-producing call spine or inside a
/// separately validated index expression. The parent owns the bounded-call and
/// non-rebinding proof for the latter.
pub(super) fn transparent_assignment_target_effect_is_structural(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => {
            transparent_assignment_target_effect_is_structural(program, *inner)
        }
        ExpressionNode::Indexed(_) => true,
        ExpressionNode::Member(member) => {
            transparent_assignment_target_effect_is_structural(program, member.receiver)
        }
        ExpressionNode::Call(_) => true,
        _ => false,
    }
}
