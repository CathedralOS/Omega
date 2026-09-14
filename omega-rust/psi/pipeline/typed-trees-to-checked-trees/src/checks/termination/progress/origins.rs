//! Adapt exact value origins to the field-only progress subject surface.

use super::{FlowCallFact, FlowFacts, FlowStateFact, ProgressSubject};
use crate::flow::{self, CanonicalPlace};
use facts::{PlaceRoot, PlaceSegment};
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::statement::{
    StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode,
};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use typed_trees::{TypedTrees, machine::Machine};

#[cfg(test)]
mod tests;

pub(super) fn at_call(
    program: &TypedTrees,
    flow: &FlowFacts,
    machine: &Machine,
    state: &FlowStateFact,
    call: &FlowCallFact,
    subject: ProgressSubject,
) -> Option<ProgressSubject> {
    let mut place = CanonicalPlace {
        root: PlaceRoot::Symbol(subject.root),
        segments: Vec::new(),
    };
    for projection in subject.projections {
        flow::push_field_place_segments(program, &mut place.segments, projection);
    }
    // The shared backward trace keeps value-position call results opaque;
    // progress supplies the one producer it can prove exactly: an owned
    // helper result substitutes the caller argument place the callee's
    // returned expression establishes, then the trace keeps walking before
    // that store.
    let place = flow::value_origin_at_call_resolving(
        program,
        flow,
        machine,
        state,
        call,
        place,
        |state, statement_index, call, relative| {
            call_result_value_place(program, state, statement_index, call, relative, 16)
        },
    )?;
    super::subject_from_place(place.root, &place.segments)
}

/// The exact caller-side place an owned call result arrived from, proven only
/// when the callee is a nongeneric checked body whose returned expression
/// resolves to a frozen input projection. Receivers, generic or evidence
/// arguments, unresolved dispatch, reference-capable inputs, and mutable
/// bindings stay opaque: a declared result type or a matching spelling is
/// never evidence of a caller identity.
fn call_result_value_place(
    program: &TypedTrees,
    state: &FlowStateFact,
    statement_index: usize,
    call: &TableCallExpression,
    result_relative: &[PlaceSegment],
    depth: usize,
) -> Option<CanonicalPlace> {
    if depth == 0 {
        return None;
    }
    let callee_state = crate::semantic_calls::find_state(program, call.target_symbol)?;
    let callee = program.machines().iter().find(|candidate| {
        program
            .machine_states(candidate)
            .iter()
            .any(|state| state.symbol == callee_state.symbol)
    })?;
    if callee.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !callee.body_is_present
        || !callee.lifetime_parameters.is_empty()
        || !program.machine_type_parameters(callee).is_empty()
        || call.receiver.is_valid() != callee.attached_data.is_some()
        || !call.machine_arguments.is_empty()
        || !call.evidence_arguments.is_empty()
        || call.static_requirement_dispatch.is_some()
        || call.quotient_operation.is_some()
        || call.private_layout_operation.is_some()
    {
        return None;
    }
    let statements = program
        .statement_table
        .statements(callee_state.statement_nodes);
    let (terminal, prefix) = statements.split_last()?;
    let result = match terminal {
        StatementNode::Expression(result) => *result,
        StatementNode::Transition(transition)
            if transition.guard == TransitionGuardNode::Always
                && transition.exit == TransitionExit::Ordinary
                && !transition.continuation.is_valid() =>
        {
            let TransitionTargetNode::Value(result) =
                program.statement_table.transition_target(transition.target)
            else {
                return None;
            };
            *result
        }
        _ => return None,
    };
    // An unresolved control-flow or binding route cannot select which input
    // supplied the result; only the single linear prefix qualifies.
    if prefix
        .iter()
        .any(|statement| matches!(statement, StatementNode::Transition(_)))
    {
        return None;
    }
    let returned = callee_value_place(program, prefix, result, 16)?;
    let PlaceRoot::Symbol(root) = returned.root else {
        return None;
    };
    let parameters = program.state_parameters(callee_state);
    let parameter = parameters
        .iter()
        .find(|parameter| parameter.symbol == root)?;
    if parameter.is_mutable || !frozen_input_reference(program, parameter.type_reference) {
        return None;
    }
    let actual = if parameter.is_self {
        call.receiver
    } else {
        let arguments = program.expression_table.expression_handles(call.arguments);
        if arguments.len()
            != parameters
                .iter()
                .filter(|parameter| !parameter.is_self)
                .count()
        {
            return None;
        }
        *arguments.get(
            parameters
                .iter()
                .filter(|parameter| !parameter.is_self)
                .position(|candidate| candidate.symbol == root)?,
        )?
    };
    let mut source = match flow::canonical_place_from_expression_in_state(
        program,
        state.state_symbol,
        statement_index,
        actual,
    )? {
        source if matches!(source.root, PlaceRoot::Symbol(_)) => source,
        CanonicalPlace {
            root: PlaceRoot::Expression(expression),
            segments,
        } => {
            // An argument that is itself a call result keeps tracing through
            // that callee's own returned expression, with the callee
            // projection and the subject's remainder appended in order.
            let ExpressionNode::Call(nested) = program.expression_table.expression(expression)
            else {
                return None;
            };
            let mut relative = segments;
            relative.extend_from_slice(&returned.segments);
            relative.extend_from_slice(result_relative);
            return call_result_value_place(
                program,
                state,
                statement_index,
                nested,
                &relative,
                depth - 1,
            );
        }
        _ => return None,
    };
    source.segments.extend_from_slice(&returned.segments);
    source.segments.extend_from_slice(result_relative);
    Some(source)
}

/// The returned expression's exact place in callee space. Only a parameter or
/// an immutable local traced to its initializer qualifies; every other root
/// (an opaque expression, a call result, a mutable slot) stays unproven.
fn callee_value_place(
    program: &TypedTrees,
    prefix: &[StatementNode],
    expression: ExpressionHandle,
    depth: usize,
) -> Option<CanonicalPlace> {
    if depth == 0 {
        return None;
    }
    let place = flow::canonical_place_from_expression(program, expression)?;
    let PlaceRoot::Symbol(root) = place.root else {
        return None;
    };
    let Some(local) = prefix.iter().find_map(|statement| match statement {
        StatementNode::LocalData(local) if local.symbol == root => Some(local),
        _ => None,
    }) else {
        return Some(place);
    };
    if local.is_mutable {
        return None;
    }
    let mut source = callee_value_place(program, prefix, local.initial_value, depth - 1)?;
    source.segments.extend_from_slice(&place.segments);
    Some(source)
}

/// A callee reads a frozen input but cannot change which storage it names:
/// an immutable owned binding or a shared reference whose referent is
/// read-only for the whole call. `&mut`, write-only, and mutable bindings can
/// rebind or rewrite the returned path, so they carry no exact caller
/// provenance.
fn frozen_input_reference(program: &TypedTrees, mut reference: TypeReferenceHandle) -> bool {
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(reference)
    {
        reference = *base_type;
    }
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Reference { access, .. } => {
            *access == language_semantics::ReferenceAccess::Shared
        }
        _ => true,
    }
}
