//! Builtin meaning required before interpreting place selectors as storage geometry.

use super::{declared_indexed_projection_type, declared_place_type_raw};
use language_core::OperatorSpelling;
use language_semantics::declaration_selection::CollectionMeasure;
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
    if CollectionMeasure::from_authored_spelling(member.member.as_str())
        != Some(CollectionMeasure::Length)
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

/// The constant extent a `.len` observation reads when the receiver's
/// declared shape is statically countable: a fixed array names its declared
/// length, and a borrow or reference spelled around such a place loans the
/// same extent through its shell. A runtime-length slice keeps the
/// observation symbolic — callers then discharge the bound through the
/// carried-fact channels as before.
pub fn declared_collection_extent(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    receiver: ExpressionHandle,
) -> Option<usize> {
    // `declared_place_type_raw` already reads through `&`/`&mut` spellings to
    // the loaned place's declared type, so `(&mut control).len` on a fixed
    // field resolves the same extent `control.len` does.
    let declared = declared_place_type_raw(program, machine, state, receiver)?;
    let declared = super::unwrapped_type_reference(program, declared)?;
    match program.type_reference_table.type_reference(declared) {
        typed_trees::types::TypeReferenceNode::FixedArray {
            length: typed_trees::types::FixedArrayLength::Literal(extent),
            ..
        } => Some(*extent),
        _ => None,
    }
}

/// Resolve the indexed geometry of a readable subslice occurrence. This
/// query grants no access: each consumer retains the authored borrow and
/// checks its access against the requested view and source loan. Write-only
/// borrowing cannot supply the length observation required by narrowing.
pub fn subslice_expression(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let operation = match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(borrow)
            if matches!(
                borrow.access,
                language_core::ReferenceAccess::Shared | language_core::ReferenceAccess::Mutable
            ) =>
        {
            borrow.target
        }
        ExpressionNode::Borrow(_) => return None,
        _ => expression,
    };
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(operation) else {
        return None;
    };
    matches!(program.expression_table.expression(indexed.index),
        ExpressionNode::Range(range) if !range.end_inclusive)
    .then_some(operation)
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
        program.expression_table.source_span(expression),
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

/// The selector expressions of an assignment target's indexed steps, from
/// the target inward: `a[i][j]` yields `[j, i]`, `a[i].f` yields `[i]`, and
/// `a.f` yields none. A range selector names a view rather than an element,
/// so it contributes no selector. Position `depth` in the result is the
/// `AssignmentIndex { depth }` coordinate a checked assignment evaluates for
/// that step, before its value; producers and consumers of those rows share
/// this one walk.
pub fn assignment_target_selectors(
    program: &TypedTrees,
    target: ExpressionHandle,
) -> Vec<ExpressionHandle> {
    let mut selectors = Vec::new();
    let mut cursor = target;
    for _ in 0..128 {
        if !program.expression_table.expression_is_valid(cursor) {
            break;
        }
        match program.expression_table.expression(cursor) {
            ExpressionNode::Indexed(indexed) => {
                if !matches!(
                    program.expression_table.expression(indexed.index),
                    ExpressionNode::Range(_)
                ) {
                    selectors.push(indexed.index);
                }
                cursor = indexed.collection;
            }
            ExpressionNode::Member(member) => cursor = member.receiver,
            _ => break,
        }
    }
    selectors
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
            if !builtin_coordinates(program, machine, state, indexed.collection, depth + 1) {
                return false;
            }
            if let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index)
            {
                // A window keeps its collection's backing rather than
                // selecting an element type. Authenticate the range operator
                // and each present bound before interpreting that geometry.
                let collection =
                    declared_place_type_raw(program, machine, state, indexed.collection)
                        .and_then(|reference| super::unwrapped_type_reference(program, reference));
                return collection.is_some_and(|reference| {
                    matches!(
                        program.type_reference_table.type_reference(reference),
                        typed_trees::types::TypeReferenceNode::Slice { .. }
                            | typed_trees::types::TypeReferenceNode::FixedArray { .. }
                    )
                }) && has_builtin_subslice_meaning(program, machine, state, expression)
                    && [range.start, range.end].into_iter().all(|bound| {
                        !bound.is_valid()
                            || crate::has_builtin_bound_expression_meaning(
                                program, machine, state, bound,
                            )
                    });
            }
            if declared_indexed_projection_type(program, machine, state, expression).is_none() {
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
                program.expression_table.source_span(expression),
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
