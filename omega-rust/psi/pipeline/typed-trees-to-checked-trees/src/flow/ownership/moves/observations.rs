use super::*;

/// Observing a place evaluates its address and calls, without moving the place.
pub(in crate::flow::ownership) fn append(
    program: &typed_trees::TypedTrees,
    sink: &mut DirectMoveEventSink<'_>,
    state: SymbolHandle,
    statement: usize,
    expression: ExpressionHandle,
    source: FlowOwnershipEventSource,
) {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(_) => {}
        ExpressionNode::Member(member) => {
            append(program, sink, state, statement, member.receiver, source)
        }
        ExpressionNode::Indexed(indexed) => {
            if append_indexed_operands(program, sink, state, statement, expression, source)
                .is_some()
            {
                return;
            }
            append(program, sink, state, statement, indexed.collection, source);
            append_move_events_for_expression(
                program,
                sink,
                state,
                statement,
                indexed.index,
                source,
            );
        }
        ExpressionNode::Cast(cast)
            if matches!(
                cast.form,
                language_core::cast_form::CastForm::RecastShared
                    | language_core::cast_form::CastForm::RecastMutable
            ) =>
        {
            append(program, sink, state, statement, cast.value, source);
        }
        ExpressionNode::Borrow(borrow) => {
            append(program, sink, state, statement, borrow.target, source)
        }
        _ => append_move_events_for_expression(program, sink, state, statement, expression, source),
    }
}

/// A selected index operator evaluates ordinary parameter-aligned operands.
/// Only intrinsic place indexing may implicitly observe its collection.
/// The result of a consuming collection operator is fresh, not a projection
/// that could transfer that same collection a second time.
pub(super) fn append_indexed_operands(
    program: &typed_trees::TypedTrees,
    sink: &mut DirectMoveEventSink<'_>,
    state: SymbolHandle,
    statement: usize,
    expression: ExpressionHandle,
    source: FlowOwnershipEventSource,
) -> Option<bool> {
    let operator_symbol = selected_operator(sink, state, statement, expression)?;
    let operator = typed_trees::operator::declaration_by_symbol(program, operator_symbol)?;
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        return None;
    };
    let operands = match program.expression_table.expression(indexed.index) {
        ExpressionNode::Range(range) => [indexed.collection, range.start, range.end],
        _ => [
            indexed.collection,
            indexed.index,
            ExpressionHandle::invalid(),
        ],
    };
    let argument_count = if matches!(
        program.expression_table.expression(indexed.index),
        ExpressionNode::Range(_)
    ) {
        2
    } else {
        1
    };
    let policy = operator_call_ownership_policy(program, Some(operator), argument_count, true);
    for (ordinal, operand) in operands.into_iter().enumerate() {
        if !operand.is_valid() {
            continue;
        }
        let transfers = if ordinal == 0 {
            policy.receiver_transfers()
        } else {
            policy.positional_transfers(ordinal - 1)
        };
        if transfers {
            append_move_events_for_expression(program, sink, state, statement, operand, source);
        } else {
            append(program, sink, state, statement, operand, source);
        }
    }
    Some(policy.receiver_transfers())
}

pub(super) fn selected_operator(
    sink: &DirectMoveEventSink<'_>,
    state: SymbolHandle,
    statement: usize,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    sink.operators.resolved_uses().find_map(|operator_use| {
        (operator_use.expression == expression
            && matches!(operator_use.origin, checked_trees::CheckedValueOrigin::StateStatement {
                state_symbol, statement_index, ..
            } if state_symbol == state && statement_index == statement))
        .then_some(operator_use.selected_operator_symbol)
    })
}

/// Targetless collection views borrow existing storage. Nominal/operator calls
/// are resolved before this intrinsic path is considered.
pub(super) fn append_builtin_collection_view(
    program: &typed_trees::TypedTrees,
    sink: &mut DirectMoveEventSink<'_>,
    state: SymbolHandle,
    statement: usize,
    call: &typed_trees::expression::TableCallExpression,
    source: FlowOwnershipEventSource,
) -> bool {
    if !matches!(call.target.as_str(), "as_slice" | "as_mut_slice")
        || call.target_symbol.is_valid()
        || !call.arguments.is_empty()
        || !call.evidence_arguments.is_empty()
        || !call.machine_arguments.is_empty()
        || call.static_requirement_dispatch.is_some()
        || call.quotient_operation.is_some()
        || call.private_layout_operation.is_some()
    {
        return false;
    }
    let Some(mut reference) =
        expression_type_reference_in_state(program, state, statement, call.receiver)
    else {
        return false;
    };
    for _ in 0..program.type_reference_table.type_reference_count() {
        if !program
            .type_reference_table
            .contains_type_reference(reference)
        {
            return false;
        }
        match program.type_reference_table.type_reference(reference) {
            typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
                reference = *referee
            }
            typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                reference = *base_type
            }
            typed_trees::types::TypeReferenceNode::FixedArray { .. }
            | typed_trees::types::TypeReferenceNode::Slice { .. } => {
                append(program, sink, state, statement, call.receiver, source);
                return true;
            }
            _ => return false,
        }
    }
    false
}
