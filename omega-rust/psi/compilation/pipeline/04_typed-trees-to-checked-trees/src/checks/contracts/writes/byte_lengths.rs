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
    program: &dyn ::validation::CheckingInput,
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
            let target = program.state_by_symbol(call.target_symbol)?;
            byte_capacity(program, target.return_type)
        }
        _ => {
            if let Some(place) = validation::canonical_place_from_expression_in_state(
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
                validation::field_domains::attached_data_field_type(program, machine, expression)
                    .or_else(|| {
                    validation::field_domains::direct_state_place_type_reference(
                        program, state, expression,
                    )
                })?;
            let carrier = byte_capacity(program, field_type);
            let observed = crate::checks::current_scalar_values::byte_length_bound(
                program,
                state,
                statement_index,
                expression,
            )
            .and_then(|bound| usize::try_from(bound).ok());
            match (carrier, observed) {
                (Some(carrier), Some(observed)) => Some(carrier.min(observed)),
                (carrier, observed) => carrier.or(observed),
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct KnownByteLength {
    place: validation::CanonicalPlace,
    max_length: usize,
}

/// Compute the straight-line byte-length facts reaching `statement_index`.
/// This is deliberately born-conservative: a call/assembly/transition or a
/// value-position call clears the summary because it may mutate an aliased
/// place. Ordinary assignments retain disjoint facts, invalidate overlapping
/// places, and publish the assigned value's new maximum length.
pub(super) fn known_byte_lengths_before(
    program: &dyn ::validation::CheckingInput,
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
            StatementNode::Assembly(_) => {}
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
                if let Some(place) = validation::canonical_place_from_expression_in_state(
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
    program: &dyn ::validation::CheckingInput,
    known: &mut Vec<KnownByteLength>,
    written: &validation::CanonicalPlace,
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
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    if !expression.is_valid() {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(_) => true,
        ExpressionNode::Match(dispatch) => {
            program.expression_table.match_deep_any(dispatch, |handle| {
                expression_contains_value_call(program, handle)
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
        typed_trees::literal_expression_pattern!() => false,
    }
}

/// A byte descriptor may retain a fixed backing bound without owning it.
/// The generic array-extent query keeps its separate ranking/geometry role.
fn byte_capacity(
    program: &dyn ::validation::CheckingInput,
    reference: typed_trees::types::TypeReferenceHandle,
) -> Option<usize> {
    match validation::byte_sequence_carrier(program, reference, &[]) {
        Some(
            checked_trees::CheckedByteSequenceCarrier::BoundedOwned { capacity }
            | checked_trees::CheckedByteSequenceCarrier::BoundedView { capacity, .. },
        ) => usize::try_from(capacity).ok(),
        Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. }) => None,
        None => validation::field_domains::type_reference_fixed_array_capacity(program, reference),
    }
}
