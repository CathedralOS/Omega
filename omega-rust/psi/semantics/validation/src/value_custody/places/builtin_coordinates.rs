//! Builtin meaning required before interpreting place selectors as storage geometry.

use super::{declared_indexed_projection_type, declared_place_type_raw};
use language_core::OperatorSpelling;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;

/// Recognize collection metadata through the receiver's structural type, not
/// through a same-spelled nominal field or accessor.
pub fn collection_length_receiver(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let ExpressionNode::Member(member) = program.expression_table.expression(expression) else {
        return None;
    };
    if member.member.as_str() != "len"
        || member.member_symbol.is_valid()
        || member.case_variant.is_some()
    {
        return None;
    }
    let receiver = declared_place_type_raw(program, machine, state, member.receiver)?;
    let receiver = super::unwrapped_type_reference(program, receiver)?;
    matches!(
        program.type_reference_table.type_reference(receiver),
        typed_trees::types::TypeReferenceNode::Slice { .. }
            | typed_trees::types::TypeReferenceNode::FixedArray { .. }
    )
    .then_some(member.receiver)
}

/// Recognize exclusive builtin subslicing without granting bounds, source
/// custody, or permission to use the resulting view.
pub fn has_builtin_subslice_meaning(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> bool {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        return false;
    };
    let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index) else {
        return false;
    };
    if range.end_inclusive {
        return false;
    }
    let operands = [
        declared_place_type_raw(program, machine, state, indexed.collection),
        declared_place_type_raw(program, machine, state, range.start),
        declared_place_type_raw(program, machine, state, range.end),
    ];
    typed_trees::operator::resolve_indexed_spelling_for_operands(
        program,
        OperatorSpelling::Range,
        &operands,
    )
    .is_empty()
        && typed_trees::operator::has_builtin_spelled_expression_meaning(
            program,
            machine.symbol,
            expression,
            OperatorSpelling::Range,
            &operands,
        )
}

/// Check only the operation meaning of a place spine. Consumers separately
/// establish declaration identity, reference origins, bounds, and authority.
/// A dynamic builtin selector may pass without establishing a fixed index.
pub fn place_has_builtin_coordinates(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> bool {
    builtin_coordinates(program, machine, state, expression, 0)
}

fn builtin_coordinates(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    depth: usize,
) -> bool {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(_) => true,
        ExpressionNode::Borrow(borrow) => {
            builtin_coordinates(program, machine, state, borrow.target, depth + 1)
        }
        ExpressionNode::Member(member) => {
            builtin_coordinates(program, machine, state, member.receiver, depth + 1)
        }
        ExpressionNode::Indexed(indexed) => {
            // Check the collection spine before contextual projection. This
            // rules out calls and authored indexing beneath a later index.
            if !builtin_coordinates(program, machine, state, indexed.collection, depth + 1)
                || declared_indexed_projection_type(program, machine, state, expression).is_none()
            {
                return false;
            }
            let operands = [
                declared_place_type_raw(program, machine, state, indexed.collection).or_else(
                    || {
                        declared_indexed_projection_type(
                            program,
                            machine,
                            state,
                            indexed.collection,
                        )
                    },
                ),
                declared_place_type_raw(program, machine, state, indexed.index),
            ];
            // Indexed syntax has implicit shared collection adaptation. The
            // ordinary custody query alone does not perform that matching.
            typed_trees::operator::resolve_indexed_spelling_for_operands(
                program,
                OperatorSpelling::Index,
                &operands,
            )
            .is_empty()
                && typed_trees::operator::has_builtin_spelled_expression_meaning(
                    program,
                    machine.symbol,
                    expression,
                    OperatorSpelling::Index,
                    &operands,
                )
                && crate::has_builtin_bound_expression_meaning(
                    program,
                    machine,
                    state,
                    indexed.index,
                )
        }
        _ => false,
    }
}
