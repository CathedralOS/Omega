use crate::flow::CanonicalPlace;
use crate::flow::FlowOwnershipEventSource;
use crate::flow::expression_type_reference_in_state;
use crate::flow::ownership::DirectMoveEventSink;
use crate::flow::ownership::append_move_events_for_expression;
use crate::flow::ownership::moves::operator_call_ownership_policy;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use language_core::ReferenceAccess;
use symbols::SymbolHandle;

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
            && operator_use.occurrence == checked_trees::CheckedOperatorOccurrence::Expression
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

/// Whether one by-value use of `place` is a detached observation rather than
/// an ownership transfer out of borrowed storage.
///
/// Two reaches admit a stable-observable copy while every other reach keeps
/// its ordinary move or borrowed-window behavior:
///
/// - A chain that crosses only shared (`&`) loans and contains an index hop
///   reads the element by copy: `view[index]` on `&[T]` cannot move the
///   element out of the shared loan, so element access produces a detached
///   value and leaves the borrowed storage whole. Field and case projections
///   keep their ordinary place ownership; only the indexed element read
///   detaches.
/// - A call through a `Service<R>`/boundary-trait receiver receives a
///   caller-owned ABI copy of each stable borrowed argument: `self.foreign
///   .call(self.field)` resolves to a boundary trait signature whose provider
///   sits behind an opaque service handle, so it observes `self.field`
///   instead of extracting it through the `&mut` receiver, and the caller's
///   loan returns intact. Direct provider calls — `boundary machine`, `via`,
///   admission, or requirement targets — still consume custody.
///
/// A `&write` link never supplies readable content, and a `&mut` reach keeps
/// its move-and-restore window outside the service-seam case, so forged or
/// custody-carrying paths still reject.
pub(super) fn detached_borrowed_copy_admitted(
    program: &typed_trees::TypedTrees,
    state: SymbolHandle,
    statement: usize,
    place: &CanonicalPlace,
    source: FlowOwnershipEventSource,
) -> bool {
    let facts::PlaceRoot::Symbol(root) = place.root else {
        return false;
    };
    let Some(projected) =
        crate::flow::canonical_place_type_reference(program, state, statement, place)
    else {
        return false;
    };
    // Only recursively stable contents may be duplicated under observation:
    // references, mutation-capable loans, nominal cleanup, linear carriers,
    // and unclassified storage keep their real transfer obligation.
    if !validation::has_stable_observable_contents(program, projected) {
        return false;
    }

    let mut has_reference = false;
    let mut has_exclusive = false;
    let mut has_write_only = false;
    let mut note = |access: Option<ReferenceAccess>| {
        if let Some(access) = access {
            has_reference = true;
            has_exclusive |= access.is_exclusive();
            has_write_only |= access == ReferenceAccess::WriteOnly;
        }
    };

    // A `self`-rooted place crosses the state receiver's loan; the machine
    // symbol and the `self` parameter are the two spellings of that root.
    if let Some(state_decl) = crate::semantic_calls::find_state(program, state)
        && let Some(self_parameter) = program
            .state_parameters(state_decl)
            .iter()
            .find(|parameter| parameter.is_self)
    {
        let machine_symbol = program.symbols.get(state_decl.symbol).parent;
        if root == machine_symbol || root == self_parameter.symbol {
            note(reference_access(program, self_parameter.type_reference));
        }
    }
    // The root place and each proper prefix may each cross a reference.
    for length in 0..place.segments.len() {
        let prefix = CanonicalPlace {
            root: place.root,
            segments: place.segments[..length].to_vec(),
        };
        note(
            crate::flow::canonical_place_type_reference(program, state, statement, &prefix)
                .and_then(|reference| reference_access(program, reference)),
        );
    }
    if !has_reference || has_write_only {
        return false;
    }
    // A purely shared reach detaches the indexed element read only.
    if !has_exclusive
        && place.segments.iter().any(|segment| {
            matches!(
                segment,
                facts::PlaceSegment::FixedIndex { .. } | facts::PlaceSegment::Index { .. }
            )
        })
    {
        return true;
    }
    // A service-receiver invocation detaches its stable arguments at the ABI
    // seam; the caller's custody stays whole.
    let FlowOwnershipEventSource::Call { target_symbol, .. } = source else {
        return false;
    };
    call_target_is_service_signature(program, target_symbol)
}

/// Whether `target_symbol` names a machine signature of a `boundary` trait —
/// the resolved target of a call through an opaque `Service<R>`/boundary
/// receiver. The provider behind that handle is not a custody participant:
/// the seam marshals a caller-owned copy of each argument, so a stable
/// borrowed argument is observed, never extracted.
///
/// Every other target keeps its transfer. A `boundary machine`, `via`
/// realization, or admission claim is a provider contract: its by-value
/// parameters consume custody like a checked body, so a borrowed argument
/// still opens the move-and-restore window (and the boundary-call fence
/// rejects an invocation that would span the hole). Requirement slots and
/// machine parameters resolve to provider contracts the same way.
fn call_target_is_service_signature(
    program: &typed_trees::TypedTrees,
    target_symbol: SymbolHandle,
) -> bool {
    // A trait-bound machine parameter reuses its bound trait's signature
    // symbols, so the same signature identity covers `M::op(...)` calls.
    let signature_symbol = program
        .machine_parameter_signature(target_symbol)
        .map_or(target_symbol, |(_, signature)| signature.symbol);
    program.traits().iter().any(|definition| {
        definition.is_boundary
            && program
                .trait_machine_signatures(definition)
                .iter()
                .any(|signature| signature.symbol == signature_symbol)
    })
}

/// The access of a possibly-constrained reference type.
fn reference_access(
    program: &typed_trees::TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Option<ReferenceAccess> {
    match program.type_reference_table.type_reference(type_reference) {
        typed_trees::types::TypeReferenceNode::Reference { access, .. } => Some(*access),
        typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            reference_access(program, *base_type)
        }
        _ => None,
    }
}
