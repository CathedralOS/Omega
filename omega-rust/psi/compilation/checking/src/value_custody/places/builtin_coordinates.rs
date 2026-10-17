//! Builtin meaning required before interpreting place selectors as storage geometry.

use super::{declared_indexed_projection_type, declared_place_type_raw};
use language_core::OperatorSpelling;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use typed_trees::machine::Machine;
use typed_trees::state::State;

/// Recognize collection metadata through the receiver's structural type, not
/// through a same-spelled nominal field or accessor.
pub fn collection_length_receiver(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let receiver = program
        .expression_table
        .authored_length_receiver(expression)?;
    let receiver_type = declared_place_type_raw(program, machine, state, receiver)?;
    let receiver_type = super::unwrapped_type_reference(program, receiver_type)?;
    matches!(
        program.type_reference_table.type_reference(receiver_type),
        typed_trees::types::TypeReferenceNode::Slice { .. }
            | typed_trees::types::TypeReferenceNode::FixedArray { .. }
    )
    .then_some(receiver)
}

/// The constant extent a `.len` observation reads when the receiver's
/// declared shape is statically countable: a fixed array names its declared
/// length, and a borrow or reference spelled around such a place loans the
/// same extent through its shell. A runtime-length slice keeps the
/// observation symbolic — callers then discharge the bound through the
/// carried-fact channels as before.
pub fn declared_collection_extent(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    receiver: ExpressionHandle,
) -> Option<usize> {
    // `declared_place_type_raw` already reads through `&`/`&mut` spellings to
    // the loaned place's declared type, so `(&mut control).len` on a fixed
    // field resolves the same extent `control.len` does.
    let declared = declared_place_type_raw(program, machine, state, receiver)?;
    fixed_array_type_length(program, declared)
}

/// A type establishes exact length only for always-full arrays. Preserve
/// constrained shells until their carrier classification is known: bounded
/// byte owners carry a live length independently of their storage capacity.
/// Expression and canonical-place observations share this rule with bounds
/// checking, so projecting or borrowing a buffer cannot turn capacity into
/// evidence that any bytes are present.
pub fn fixed_array_type_length(
    program: &dyn crate::CheckingInput,
    mut reference: typed_trees::types::TypeReferenceHandle,
) -> Option<usize> {
    loop {
        if crate::bounded_byte_buffer_capacity(program, reference).is_some() {
            return None;
        }
        match program.type_reference_table.type_reference(reference) {
            typed_trees::types::TypeReferenceNode::FixedArray {
                length: typed_trees::types::FixedArrayLength::Literal(extent),
                ..
            } => return Some(*extent),
            typed_trees::types::TypeReferenceNode::Reference { referee, .. }
            | typed_trees::types::TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => reference = *referee,
            _ => return None,
        }
    }
}

/// Resolve the indexed geometry of a readable subslice occurrence. This
/// query grants no access: each consumer retains the authored borrow and
/// checks its access against the requested view and source loan. Write-only
/// borrowing cannot supply the length observation required by narrowing.
pub fn subslice_expression(
    program: &dyn crate::CheckingInput,
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
    let indexed = program.expression_table.indexed_node(operation)?;
    matches!(
        program.expression_table.expression(indexed.index),
        ExpressionNode::Range(_)
    )
    .then_some(operation)
}

/// Recognize builtin subslicing without granting bounds, source
/// custody, or permission to use the resulting view.
pub fn has_builtin_subslice_meaning(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> bool {
    let Some(indexed) = program.expression_table.indexed_node(expression) else {
        return false;
    };
    if program.expression_table.range_node(indexed.index).is_none() {
        return false;
    }
    indexed_operation_has_builtin_meaning(
        program,
        machine,
        state,
        expression,
        declared_place_type_raw(program, machine, state, indexed.collection),
    )
}

/// Authenticate this indexed occurrence, independently of how its collection
/// was produced. A call result or an earlier declared operator may supply a
/// collection without granting storage geometry for that whole expression.
/// The caller supplies the actual collection carrier; this query establishes
/// only operator meaning, not bounds, integer landing, or loan authority.
pub fn indexed_operation_has_builtin_meaning(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    collection: Option<typed_trees::types::TypeReferenceHandle>,
) -> bool {
    let Some(indexed) = program.expression_table.indexed_node(expression) else {
        return false;
    };
    let Some(collection) = collection else {
        return false;
    };
    let mut operands = [Some(collection), None, None];
    let (spelling, operand_count) =
        if let Some(range) = program.expression_table.range_node(indexed.index) {
            operands[1] = declared_place_type_raw(program, machine, state, range.start);
            operands[2] = declared_place_type_raw(program, machine, state, range.end);
            (OperatorSpelling::Range, 3)
        } else {
            operands[1] = declared_place_type_raw(program, machine, state, indexed.index);
            (OperatorSpelling::Index, 2)
        };
    let operands = &operands[..operand_count];
    typed_trees::operator::resolve_indexed_spelling_for_operands(
        program,
        spelling,
        operands,
        program.expression_table.source_span(expression),
    )
    .is_empty()
        && typed_trees::operator::has_builtin_spelled_expression_meaning(
            program,
            machine.symbol,
            expression,
            spelling,
            operands,
        )
}

/// The source of an index-preserving whole view. This is storage geometry,
/// not borrow authority: consumers still check the selected access and loan.
/// Only builtin range meaning permits this equality. A declared operator with
/// identical bounds may return unrelated storage and establishes no such edge.
/// Omitted bounds and constant extents share this history-free judgment.
/// Captured scalar endpoints need a live fact correspondence; the declaration
/// of their initializer alone cannot establish the window's current geometry.
pub fn whole_collection_view_source(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let operation = subslice_expression(program, expression)?;
    if !has_builtin_subslice_meaning(program, machine, state, operation) {
        return None;
    }
    let indexed = program.expression_table.indexed_node(operation)?;
    let range = program.expression_table.range_node(indexed.index)?;
    let integer = |expression| {
        // Closed arithmetic still belongs to the selected numeric owner;
        // syntax-only folding could substitute a declared token machine.
        if let Some(value) = program.closed_integer_expression_value_in(expression, machine.symbol)
        {
            value.to_u64()
        } else if let Some(state) = state {
            crate::captured_integer_value(program, machine, state, expression, |_| None)
                .and_then(|value| value.to_u64())
        } else {
            None
        }
    };
    if range.start.is_valid() && integer(range.start) != Some(0) {
        return None;
    }
    if !range.end.is_valid() {
        return (!range.end_inclusive).then_some(indexed.collection);
    }
    if let Some(end) = integer(range.end)
        .and_then(|end| usize::try_from(end).ok())
        .and_then(|end| {
            if range.end_inclusive {
                end.checked_add(1)
            } else {
                Some(end)
            }
        })
        && declared_collection_extent(program, machine, state, indexed.collection) == Some(end)
    {
        return Some(indexed.collection);
    }
    None
}

/// The selector expressions of an assignment target's indexed steps, from
/// the target inward: `a[i][j]` yields `[j, i]`, `a[i].f` yields `[i]`, and
/// `a.f` yields none. A range selector names a view rather than an element,
/// so it contributes no selector. Position `depth` in the result is the
/// `AssignmentIndex { depth }` coordinate a checked assignment evaluates for
/// that step, before its value; producers and consumers of those rows share
/// this one walk.
pub fn assignment_target_selectors(
    program: &dyn crate::CheckingInput,
    target: ExpressionHandle,
) -> Vec<ExpressionHandle> {
    assignment_target_indexes(&program.expression_table, target)
        .into_iter()
        .filter_map(
            |expression| match program.expression_table.expression(expression) {
                ExpressionNode::Indexed(indexed) => Some(indexed.index),
                _ => None,
            },
        )
        .collect()
}

/// The scalar indexed occurrences along an assignment's destination, in its
/// outside-in coordinate order. An occurrence identifies its own collection
/// and bound, even when two steps use the same selector. The selector walk and
/// checked-decision consumers use this same ordering; `a[i][j]` retains the
/// outer `a[i][j]` occurrence at depth zero and `a[i]` at depth one.
pub fn assignment_target_indexes(
    expressions: &ExpressionTable,
    target: ExpressionHandle,
) -> Vec<ExpressionHandle> {
    let mut indexes = Vec::new();
    let mut cursor = target;
    for _ in 0..128 {
        if !expressions.expression_is_valid(cursor) {
            break;
        }
        match expressions.expression(cursor) {
            ExpressionNode::Indexed(indexed) => {
                if !matches!(
                    expressions.expression(indexed.index),
                    ExpressionNode::Range(_)
                ) {
                    indexes.push(cursor);
                }
                cursor = indexed.collection;
            }
            ExpressionNode::Member(member) => cursor = member.receiver,
            _ => break,
        }
    }
    indexes
}

/// Check only the operation meaning of a place spine. Consumers separately
/// establish declaration identity, reference origins, bounds, and authority.
/// A dynamic builtin selector may pass without establishing a fixed index.
pub fn place_has_builtin_coordinates(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
) -> bool {
    builtin_coordinates(program, machine, state, expression, 0)
}

/// A value spine (a fresh call or constructor result) has no place type for
/// `place_has_builtin_coordinates` to name. Its walker supplies the collection
/// carrier it reached, and the same `[]` spelling and bound authentication
/// decide whether this selection is the builtin element coordinate.
pub fn value_element_has_builtin_coordinate(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    occurrence: ExpressionHandle,
    collection: typed_trees::types::TypeReferenceHandle,
) -> bool {
    let Some(indexed) = program.expression_table.indexed_node(occurrence) else {
        return false;
    };
    if program.expression_table.range_node(indexed.index).is_some() {
        return false;
    }
    indexed_operation_has_builtin_meaning(program, machine, state, occurrence, Some(collection))
        && crate::has_builtin_bound_expression_meaning(program, machine, state, indexed.index)
}

/// The element type when `symbol` names a state parameter whose declared
/// type is a mutable view over bytes (`&mut [u8]`). Other collection shapes
/// and other access permissions have no builtin byte-write geometry.
pub fn mutable_byte_slice_parameter_element(
    program: &dyn crate::CheckingInput,
    state: &State,
    symbol: symbols::SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    use typed_trees::types::TypeReferenceNode;
    let parameter = program.state_parameter_by_symbol(state, symbol)?;
    let TypeReferenceNode::Reference {
        referee,
        access: language_core::ReferenceAccess::Mutable,
        ..
    } = program
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        return None;
    };
    let TypeReferenceNode::Slice { element_type } =
        program.type_reference_table.type_reference(*referee)
    else {
        return None;
    };
    (program.primitive_type_reference(*element_type) == Some(typed_trees::types::PrimitiveType::U8))
        .then_some(*element_type)
}

fn builtin_coordinates(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: Option<&State>,
    expression: ExpressionHandle,
    depth: usize,
) -> bool {
    let Some(node) = program
        .expression_table
        .expression_at_depth(expression, depth)
    else {
        return false;
    };
    match node {
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
            if let Some(range) = program.expression_table.range_node(indexed.index) {
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
            typed_trees::operator::builtin_index_read_is_spelled(
                program,
                machine.symbol,
                expression,
                &operands,
            ) && crate::has_builtin_bound_expression_meaning(program, machine, state, indexed.index)
        }
        _ => false,
    }
}
