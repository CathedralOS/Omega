//! Statically-known byte lengths: literals, concatenations, reaching writes,
//! and declared `[u8; N]` capacities feeding the bounded-carrier length-fits
//! check.

use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;

/// bound is statically known, or `None` when it cannot be bounded. Underwrites
/// the length-fits check on writes into a bounded `[u8; N]` text carrier:
///   * a string literal contributes its exact byte length;
///   * a concatenation `a + b` contributes the sum of its operands' bounds;
///   * a place with a straight-line reaching write contributes that write's
///     running bound; otherwise an owned carrier read contributes its declared
///     `[u8; N]` capacity;
///   * a value call contributes its declared bounded-carrier return capacity.
///
/// Anything else -- a `&[u8]` view source (no inline capacity) or an unresolved
/// local -- is unbounded, yielding `None` (conservatively rejected by the
/// caller). The `known_lengths` input is a conservative straight-line
/// reaching-definition summary: calls and other opaque statements clear it,
/// and every write invalidates overlapping places before publishing its new
/// bound. Thus in-place append uses the proven current length when one reaches
/// the statement, never merely the storage capacity.
pub(super) fn static_max_byte_length(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
    known_lengths: &[KnownByteLength],
) -> Option<usize> {
    if !expression.is_valid() {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::String(literal) => Some(literal.len()),
        ExpressionNode::Binary(binary)
            if binary.operator == typed_trees::expression::BinaryOperator::Add =>
        {
            let left = static_max_byte_length(
                program,
                machine,
                state,
                statement_index,
                binary.left,
                known_lengths,
            )?;
            let right = static_max_byte_length(
                program,
                machine,
                state,
                statement_index,
                binary.right,
                known_lengths,
            )?;
            Some(left.saturating_add(right))
        }
        ExpressionNode::Borrow(inner) => static_max_byte_length(
            program,
            machine,
            state,
            statement_index,
            inner.target,
            known_lengths,
        ),
        ExpressionNode::Call(call) => {
            let target = crate::semantic::calls::find_state(program, call.target_symbol)?;
            crate::facts::field_domain::type_reference_fixed_array_capacity(
                program,
                target.return_type,
            )
        }
        _ => {
            if let Some(place) = crate::flow::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                statement_index,
                expression,
            ) && let Some(known) = known_lengths
                .iter()
                .rev()
                .find(|known| known.place == place)
            {
                return Some(known.max_length);
            }
            let field_type =
                crate::facts::field_domain::attached_data_field_type(program, machine, expression)
                    .or_else(|| {
                        crate::facts::field_domain::direct_state_place_type_reference(
                            program, state, expression,
                        )
                    })?;
            crate::facts::field_domain::type_reference_fixed_array_capacity(program, field_type)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct KnownByteLength {
    place: crate::flow::CanonicalPlace,
    max_length: usize,
}

/// Compute the straight-line byte-length facts reaching `statement_index`.
/// This is deliberately born-conservative: a call/assembly/transition or a
/// value-position call clears the summary because it may mutate an aliased
/// place. Ordinary assignments retain disjoint facts, invalidate overlapping
/// places, and publish the assigned value's new maximum length.
pub(super) fn known_byte_lengths_before(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
) -> Vec<KnownByteLength> {
    let mut known = Vec::new();
    for (index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
        .enumerate()
    {
        match statement {
            StatementNode::Assignment(assignment) => {
                if expression_contains_value_call(program, assignment.value) {
                    known.clear();
                }
                let max_length = static_max_byte_length(
                    program,
                    machine,
                    state,
                    index,
                    assignment.value,
                    &known,
                );
                if let Some(place) = crate::flow::canonical_place_from_expression_in_state(
                    program,
                    state.symbol,
                    index,
                    assignment.target,
                ) {
                    forget_overlapping_lengths(program, &mut known, &place);
                    if let Some(max_length) = max_length {
                        known.push(KnownByteLength { place, max_length });
                    }
                } else {
                    known.clear();
                }
            }
            StatementNode::LocalData(local) => {
                if expression_contains_value_call(program, local.initial_value) {
                    known.clear();
                }
                let max_length = static_max_byte_length(
                    program,
                    machine,
                    state,
                    index,
                    local.initial_value,
                    &known,
                );
                if let Some(place) = crate::flow::canonical_place_from_symbol(local.symbol)
                    && let Some(max_length) = max_length
                {
                    forget_overlapping_lengths(program, &mut known, &place);
                    known.push(KnownByteLength { place, max_length });
                }
            }
            StatementNode::RootBinding(_)
            | StatementNode::AssemblyFact(_)
            | StatementNode::Call(_)
            | StatementNode::Expression(_)
            | StatementNode::Transition(_) => known.clear(),
        }
    }
    known
}

fn forget_overlapping_lengths(
    program: &typed_trees::TypedTrees,
    known: &mut Vec<KnownByteLength>,
    written: &crate::flow::CanonicalPlace,
) {
    known.retain(|candidate| {
        candidate.place.root != written.root
            || !crate::flow::canonical_place_segments_may_overlap(
                program,
                &candidate.place.segments,
                &written.segments,
            )
    });
}

fn expression_contains_value_call(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> bool {
    if !expression.is_valid() {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(_) => true,
        ExpressionNode::Match(dispatch) => {
            expression_contains_value_call(program, dispatch.subject)
                || program
                    .expression_table
                    .match_arms(dispatch.arms)
                    .iter()
                    .any(|arm| {
                        matches!(arm.pattern, typed_trees::expression::MatchPattern::Value(pattern)
                        if expression_contains_value_call(program, pattern))
                            || expression_contains_value_call(program, arm.value)
                    })
        }
        ExpressionNode::Atomic(atomic) => expression_contains_value_call(program, atomic.value),
        ExpressionNode::Binary(binary) => {
            expression_contains_value_call(program, binary.left)
                || expression_contains_value_call(program, binary.right)
        }
        ExpressionNode::Cast(cast) => expression_contains_value_call(program, cast.value),
        ExpressionNode::Indexed(indexed) => {
            expression_contains_value_call(program, indexed.collection)
                || expression_contains_value_call(program, indexed.index)
        }
        ExpressionNode::Member(member) => expression_contains_value_call(program, member.receiver),
        ExpressionNode::Borrow(inner) => expression_contains_value_call(program, inner.target),
        ExpressionNode::Unary(unary) => expression_contains_value_call(program, unary.operand),
        ExpressionNode::Range(range) => {
            expression_contains_value_call(program, range.start)
                || expression_contains_value_call(program, range.end)
        }
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| expression_contains_value_call(program, field.value)),
        ExpressionNode::ArrayLiteral(elements) => program
            .expression_table
            .expression_handles(*elements)
            .iter()
            .any(|element| expression_contains_value_call(program, *element)),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}
