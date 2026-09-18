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
    call_frames: Option<&validation::CallFrameResolver<'_>>,
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
        call_frames,
        |state, statement_index, call, relative| {
            call_result_value_place(program, state, statement_index, call, relative, 16)
        },
    )?;
    crate::checks::termination::progress::fact_subjects::subject_from_place(
        place.root,
        &place.segments,
    )
}

/// Where the argument expressions a checked helper call binds live: inside
/// the caller's own statement stream at the retained call row, or inside a
/// proven callee's transition-free prefix while the proof recurses through
/// that callee's returned expression.
#[derive(Clone, Copy)]
enum ArgumentScope<'a> {
    Caller {
        state: &'a FlowStateFact,
        statement_index: usize,
    },
    Callee {
        prefix: &'a [StatementNode],
    },
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
    call_result_place(
        program,
        ArgumentScope::Caller {
            state,
            statement_index,
        },
        call,
        result_relative,
        depth,
    )
}

/// The callee-body half of `call_result_value_place`: the same gates and the
/// same parameter mapping, with the argument's ambient scope carried so a
/// returned expression that is itself a checked call keeps proving through
/// that nested callee instead of stopping at an opaque leaf.
fn call_result_place(
    program: &TypedTrees,
    scope: ArgumentScope<'_>,
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
    // `result_relative` is the demanded path into the call result; the callee
    // trace applies it inside its own body so a returned constructor routes
    // the demand to the operand that supplied that exact field or element.
    let returned = callee_value_place(
        program,
        prefix,
        result,
        callee_state.return_type,
        result_relative,
        depth,
    )?;
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
    scope_argument_place(
        program,
        scope,
        actual,
        parameter.type_reference,
        &returned.segments,
        depth - 1,
    )
}

/// The call-bound argument's exact place in the ambient scope. The caller's
/// own stream keeps the shared backward trace, including an argument that is
/// itself a call result; inside a proven callee the argument is a callee-side
/// expression that recurses through that callee's locals and nested calls.
fn scope_argument_place(
    program: &TypedTrees,
    scope: ArgumentScope<'_>,
    actual: ExpressionHandle,
    declared_type: TypeReferenceHandle,
    relative: &[PlaceSegment],
    depth: usize,
) -> Option<CanonicalPlace> {
    match scope {
        ArgumentScope::Caller {
            state,
            statement_index,
        } => {
            match flow::canonical_place_from_expression_in_state(
                program,
                state.state_symbol,
                statement_index,
                actual,
            )? {
                mut source if matches!(source.root, PlaceRoot::Symbol(_)) => {
                    source.segments.extend_from_slice(relative);
                    Some(source)
                }
                CanonicalPlace {
                    root: PlaceRoot::Expression(expression),
                    segments,
                } => {
                    // An argument that is itself a call result keeps tracing
                    // through that callee's own returned expression, with the
                    // callee projection and the subject's remainder appended
                    // in order.
                    let ExpressionNode::Call(nested) =
                        program.expression_table.expression(expression)
                    else {
                        return None;
                    };
                    let mut nested_relative = segments;
                    nested_relative.extend_from_slice(relative);
                    call_result_place(program, scope, nested, &nested_relative, depth)
                }
                _ => None,
            }
        }
        ArgumentScope::Callee { prefix } => {
            callee_value_place(program, prefix, actual, declared_type, relative, depth)
        }
    }
}

/// The returned expression's exact place in callee space, under the demanded
/// `relative` projection into its value. A parameter or an immutable local
/// traced to its initializer qualifies, as does a nested checked call proven
/// through the same result gate; every other root (an opaque expression, a
/// mutable slot) stays unproven.
fn callee_value_place(
    program: &TypedTrees,
    prefix: &[StatementNode],
    expression: ExpressionHandle,
    declared_type: TypeReferenceHandle,
    relative: &[PlaceSegment],
    depth: usize,
) -> Option<CanonicalPlace> {
    if depth == 0 {
        return None;
    }
    // A constructor has no storage of its own: the demanded projection
    // arrives from the operand bound to that exact field or element, which
    // then proves its own origin under the same leaf rules. A dynamic index
    // or range names several operands, so it is not one exact origin.
    if !relative.is_empty()
        && matches!(
            program.expression_table.expression(expression),
            ExpressionNode::StructLiteral(_) | ExpressionNode::ArrayLiteral(_)
        )
    {
        let mut projections =
            flow::literal_value_projections(program, expression, declared_type, relative, false)?;
        if projections.len() != 1 {
            return None;
        }
        let projection = projections.remove(0);
        return callee_value_place_leaf(
            program,
            prefix,
            projection.expression,
            &projection.remaining,
            depth - 1,
        );
    }
    callee_value_place_leaf(program, prefix, expression, relative, depth)
}

/// The non-constructor leaf of `callee_value_place`. `relative` is appended
/// after the expression's own path: for `local.field` under demanded `[x]`
/// the initializer owes `[field, x]`, and a nested call owes the same
/// composed path as its result projection.
fn callee_value_place_leaf(
    program: &TypedTrees,
    prefix: &[StatementNode],
    expression: ExpressionHandle,
    relative: &[PlaceSegment],
    depth: usize,
) -> Option<CanonicalPlace> {
    if depth == 0 {
        return None;
    }
    let place = flow::canonical_place_from_expression(program, expression)?;
    let root = match place.root {
        PlaceRoot::Symbol(root) => root,
        PlaceRoot::Expression(rooted) => {
            // A returned expression that is itself a checked call arrives at
            // that callee's input the same way the outer call arrives at this
            // one's: the proof recurses through the nested callee's own
            // returned expression, keeping the projected segments.
            let ExpressionNode::Call(nested) = program.expression_table.expression(rooted) else {
                return None;
            };
            let mut nested_relative = place.segments;
            nested_relative.extend_from_slice(relative);
            return call_result_place(
                program,
                ArgumentScope::Callee { prefix },
                nested,
                &nested_relative,
                depth - 1,
            );
        }
        _ => return None,
    };
    let Some(local) = prefix.iter().find_map(|statement| match statement {
        StatementNode::LocalData(local) if local.symbol == root => Some(local),
        _ => None,
    }) else {
        let mut place = place;
        place.segments.extend_from_slice(relative);
        return Some(place);
    };
    if local.is_mutable {
        return None;
    }
    // The demanded path into the local's value applies to its initializer,
    // including a constructor bound for later return.
    let mut demanded = place.segments;
    demanded.extend_from_slice(relative);
    callee_value_place(
        program,
        prefix,
        local.initial_value,
        local.type_reference,
        &demanded,
        depth - 1,
    )
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
