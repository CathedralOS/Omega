//! call places slice of origins.
use validation::CanonicalPlace;

use super::{callee_values::*, reference_bounds::*};

use super::{FlowCallFact, FlowStateFact, ProgressSubject};
use crate::flow::{self};
use facts::{PlaceRoot, PlaceSegment};
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionExit, TransitionTargetNode};
use typed_trees::types::TypeReferenceHandle;

/// The subjects a demanded call result descends from when the callee's routes
/// DISAGREE, or `None` when they do not.
///
/// The shared backward trace carries one origin: its `resolve` closure returns
/// a single place, so a call whose arms name different inputs stays opaque to
/// it however exactly each arm is known. The arms are each exact, so the result
/// descends from one of them and the demand holds only if it holds for ALL --
/// `lineage::resolve` already turns a subject set into one premise apiece.
/// This reaches a demand that names the bound local itself; a projection the
/// trace still has to walk keeps the single-origin path.
pub(super) fn disagreeing_call_result_subjects(
    program: &dyn ::validation::CheckingInput,
    frames: &validation::CallFrameResolver<'_>,
    state: &FlowStateFact,
    call: &FlowCallFact,
    place: &CanonicalPlace,
) -> Option<Vec<ProgressSubject>> {
    let root = place.root.as_symbol()?;
    let typed_state = program.state_by_symbol(state.state_symbol)?;
    let (statement_index, local) = program
        .state_local_declaration(typed_state, root)
        .filter(|&(ordinal, _)| ordinal < call.statement_index)?;
    let initializer = program.expression_table.call_node(local.initial_value)?;
    let places = call_result_place(
        program,
        frames,
        ArgumentScope::Caller {
            state,
            statement_index,
        },
        initializer,
        &place.segments,
        16,
    )?;
    if places.len() < 2 {
        // One route is the ordinary origin the shared trace already carries.
        return None;
    }
    let mut subjects = Vec::with_capacity(places.len());
    for place in places {
        let subject = crate::checks::termination::progress::fact_subjects::subject_from_place(
            place.root,
            &place.segments,
        )?;
        if !subjects.contains(&subject) {
            subjects.push(subject);
        }
    }
    Some(subjects)
}

/// The exact caller-side place a premise-bearing call argument names. A
/// spelled place keeps its own identity; an argument that is itself a nested
/// call result — or a projection peeled off one, or a constructor operand —
/// has no storage of its own, so the same `caller_argument_place` replay that
/// resolves stored helper results proves which input supplied it. Anything
/// the replay cannot prove stays unproven rather than borrowing a same-shaped
/// root.
pub(in crate::checks::termination::progress) fn call_argument_place(
    program: &dyn ::validation::CheckingInput,
    state: &FlowStateFact,
    statement_index: usize,
    actual: ExpressionHandle,
    declared_type: TypeReferenceHandle,
    relative: &[PlaceSegment],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<CanonicalPlace> {
    let mut owned_frames = None;
    let frames = flow::shared_call_frames_or(call_frames, program, &mut owned_frames)?;
    caller_argument_place(
        program,
        frames,
        state,
        statement_index,
        actual,
        declared_type,
        relative,
        16,
    )
}

/// The demanded place a premise-bearing argument names before the premise
/// surface extracts a subject. A spelled operand through a reference leaf —
/// an indexed carrier's element, a bound view — names the same storage as
/// the referent the leaf's latest store supplied, so the boundary scan
/// rebases it here with the same bound the subject query uses. Anything the
/// scan cannot prove keeps the literal place, which the field-only premise
/// surface then refuses the way it refuses an index selector.
pub(in crate::checks::termination::progress) fn call_argument_boundary_place(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &FlowStateFact,
    bound: usize,
    place: CanonicalPlace,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<CanonicalPlace> {
    let mut owned_frames = None;
    let frames = flow::shared_call_frames_or(call_frames, program, &mut owned_frames)?;
    let resolve = |state: &FlowStateFact,
                   statement_index: usize,
                   call: &TableCallExpression,
                   relative: &[PlaceSegment]| {
        call_result_value_place(program, frames, state, statement_index, call, relative, 16)
    };
    Some(
        reference_boundary_before_statement(
            program, frames, machine, state, bound, &place, &resolve,
        )
        .unwrap_or(place),
    )
}

/// Where the argument expressions a checked helper call binds live: inside
/// the caller's own statement stream at the retained call row, or inside a
/// proven callee's transition-free prefix while the proof recurses through
/// that callee's returned expression.
#[derive(Clone, Copy)]
pub(super) enum ArgumentScope<'a> {
    Caller {
        state: &'a FlowStateFact,
        statement_index: usize,
    },
    Callee {
        body: CalleeBody<'a>,
    },
}

/// The checked callee body under proof. The write scan that keeps mutable
/// bindings honest needs the machine/state pair so the shared frame resolver
/// can name this body's may-write places; `prefix` is the transition-free
/// statement run before the returned expression.
#[derive(Clone, Copy)]
pub(super) struct CalleeBody<'a> {
    pub(super) machine: &'a Machine,
    pub(super) state: &'a State,
    pub(super) prefix: &'a [StatementNode],
}

/// The exact caller-side place an owned call result arrived from, proven only
/// when the callee is a nongeneric checked body whose returned expression
/// resolves to an input projection the body provably leaves unwritten.
/// Receivers, generic or evidence arguments, unresolved dispatch, and opaque
/// write frames stay opaque: a declared result type or a matching spelling is
/// never evidence of a caller identity.
pub(super) fn call_result_value_place(
    program: &dyn ::validation::CheckingInput,
    frames: &validation::CallFrameResolver<'_>,
    state: &FlowStateFact,
    statement_index: usize,
    call: &TableCallExpression,
    result_relative: &[PlaceSegment],
    depth: usize,
) -> Option<CanonicalPlace> {
    // The shared backward trace names ONE origin. A call whose routes
    // disagree has no single one, so it stays opaque here exactly as it did
    // before the routes were enumerated; `call_result_alternatives` is what
    // carries a disagreeing set, and only the progress lineage consumes it.
    single_route(call_result_place(
        program,
        frames,
        ArgumentScope::Caller {
            state,
            statement_index,
        },
        call,
        result_relative,
        depth,
    ))
}

pub(super) fn single_route(places: Option<Vec<CanonicalPlace>>) -> Option<CanonicalPlace> {
    let mut places = places?;
    if places.len() != 1 {
        return None;
    }
    places.pop()
}

/// The callee-body half of `call_result_value_place`: the same gates and the
/// same parameter mapping, with the argument's ambient scope carried so a
/// returned expression that is itself a checked call keeps proving through
/// that nested callee instead of stopping at an opaque leaf.
pub(super) fn call_result_place(
    program: &dyn ::validation::CheckingInput,
    frames: &validation::CallFrameResolver<'_>,
    scope: ArgumentScope<'_>,
    call: &TableCallExpression,
    result_relative: &[PlaceSegment],
    depth: usize,
) -> Option<Vec<CanonicalPlace>> {
    if depth == 0 {
        return None;
    }
    let (callee, callee_state) = validation::find_state(program, call.target_symbol)?;
    if callee.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !callee.body_is_present
        || !callee.lifetime_parameters.is_empty()
        || !program.machine_type_parameters(callee).is_empty()
        || call.receiver.is_valid() != callee.attached_data.is_some()
        || !call.carries_only_positional_arguments()
        || !call.selects_only_nominal_route()
    {
        return None;
    }
    let statements = program
        .statement_table
        .statements(callee_state.statement_nodes);
    // Every route the body can take, and the linear prefix they share. A body
    // that ends in one expression or one unconditional value transition has a
    // single route. A guarded body -- `transition flag { true -> a false -> b }`
    // -- lowers to one transition statement per arm, each carrying its own
    // guard and its own value target, and each is a route: the result is one
    // of them, so the demand holds only if EVERY arm names an exact input.
    // Statements before the first transition stay the shared prefix.
    let (prefix, routes) = match statements
        .iter()
        .position(|statement| matches!(statement, StatementNode::Transition(_)))
    {
        None => {
            let (terminal, prefix) = statements.split_last()?;
            let result = terminal.as_expression()?;
            (prefix, vec![*result])
        }
        Some(index) => {
            let (prefix, arms) = statements.split_at(index);
            let mut routes = Vec::with_capacity(arms.len());
            for statement in arms {
                // A binding route, a named successor or a continuation cannot
                // select which input supplied the result.
                let transition = statement.as_transition()?;
                if transition.exit != TransitionExit::Ordinary || transition.continuation.is_valid()
                {
                    return None;
                }
                let TransitionTargetNode::Value(result) =
                    program.statement_table.transition_target(transition.target)
                else {
                    return None;
                };
                routes.push(*result);
            }
            (prefix, routes)
        }
    };
    let body = CalleeBody {
        machine: callee,
        state: callee_state,
        prefix,
    };
    let mut places = Vec::with_capacity(routes.len());
    for result in routes {
        // One unknown route leaves the whole result unproven: an alternative
        // the trace cannot name could have supplied it.
        let place = call_route_place(
            program,
            frames,
            scope,
            call,
            callee_state,
            body,
            result,
            result_relative,
            depth,
        )?;
        if !places.contains(&place) {
            places.push(place);
        }
    }
    Some(places)
}

/// One route's result place, mapped back to the caller's actual argument.
#[allow(clippy::too_many_arguments)]
pub(super) fn call_route_place(
    program: &dyn ::validation::CheckingInput,
    frames: &validation::CallFrameResolver<'_>,
    scope: ArgumentScope<'_>,
    call: &TableCallExpression,
    callee_state: &typed_trees::state::State,
    body: CalleeBody<'_>,
    result: typed_trees::expression::ExpressionHandle,
    result_relative: &[PlaceSegment],
    depth: usize,
) -> Option<CanonicalPlace> {
    // `result_relative` is the demanded path into the call result; the callee
    // trace applies it inside its own body so a returned constructor routes
    // the demand to the operand that supplied that exact field or element.
    let mut returned = callee_value_place(
        program,
        frames,
        body,
        result,
        callee_state.return_type,
        result_relative,
        depth,
    )?;
    let Some(mut root) = returned.root.as_symbol() else {
        return None;
    };
    let parameters = program.state_parameters(callee_state);
    let mut parameter = parameters
        .iter()
        .find(|parameter| parameter.symbol == root)?;
    if parameter.is_mutable || !frozen_input_reference(program, parameter.type_reference) {
        // A mutable or write-capable binding carries exact provenance only
        // through the callee's own statement stream: replay the demanded
        // place so an exact store supplies the value the returned expression
        // names — the replacement input, not the written binding. Writes and
        // frames the trace cannot replay leave the path unproven rather than
        // borrowing root correspondence.
        returned = callee_demanded_origin(program, frames, body, returned, depth)?;
        let resolved = returned.root.as_symbol()?;
        root = resolved;
        parameter = parameters
            .iter()
            .find(|parameter| parameter.symbol == root)?;
    }
    let actual = {
        let arguments = program.expression_table.expression_handles(call.arguments);
        if arguments.len() != parameters.iter().count() {
            return None;
        }
        *arguments.get(
            parameters
                .iter()
                .position(|candidate| candidate.symbol == root)?,
        )?
    };
    scope_argument_place(
        program,
        frames,
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
pub(super) fn scope_argument_place(
    program: &dyn ::validation::CheckingInput,
    frames: &validation::CallFrameResolver<'_>,
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
        } => caller_argument_place(
            program,
            frames,
            state,
            statement_index,
            actual,
            declared_type,
            relative,
            depth,
        ),
        ArgumentScope::Callee { body } => callee_value_place(
            program,
            frames,
            body,
            actual,
            declared_type,
            relative,
            depth,
        ),
    }
}

/// The caller-scope half of `scope_argument_place`. A constructor operand has
/// no storage of its own: the callee's demanded path selects the operand that
/// supplied that exact field or element, which then proves its own origin
/// under the same leaf rules — a name keeps the shared backward trace's
/// place, and a value-position call result recurses through that callee's
/// own returned expression. A bare constructor under no demanded projection
/// names several operands, so it is not one exact origin.
pub(super) fn caller_argument_place(
    program: &dyn ::validation::CheckingInput,
    frames: &validation::CallFrameResolver<'_>,
    state: &FlowStateFact,
    statement_index: usize,
    actual: ExpressionHandle,
    declared_type: TypeReferenceHandle,
    relative: &[PlaceSegment],
    mut depth: usize,
) -> Option<CanonicalPlace> {
    let mut expression = actual;
    let mut demanded = relative.to_vec();
    loop {
        if depth == 0 {
            return None;
        }
        let place = validation::canonical_place_from_expression_in_state(
            program,
            state.state_symbol,
            statement_index,
            expression,
        )?;
        match place.root {
            PlaceRoot::Symbol(_) => {
                let mut source = place;
                source.segments.extend_from_slice(&demanded);
                return Some(source);
            }
            PlaceRoot::Expression(rooted) => {
                let mut peeled = place.segments;
                peeled.extend_from_slice(&demanded);
                match program.expression_table.expression(rooted) {
                    // An argument that is itself a call result keeps tracing
                    // through that callee's own returned expression, with the
                    // callee projection and the subject's remainder appended
                    // in order.
                    ExpressionNode::Call(nested) => {
                        return single_route(call_result_place(
                            program,
                            frames,
                            ArgumentScope::Caller {
                                state,
                                statement_index,
                            },
                            nested,
                            &peeled,
                            depth,
                        ));
                    }
                    ExpressionNode::StructLiteral(_) | ExpressionNode::ArrayLiteral(_) => {
                        if peeled.is_empty() {
                            return None;
                        }
                        // The literal's own declared type supplies the
                        // projection frame; the parameter's declared type
                        // applies only while the operand is the literal
                        // itself, not a member or element peeled off it.
                        let declared = (rooted == expression).then_some(declared_type);
                        let mut projections = flow::literal_expression_projections(
                            program,
                            rooted,
                            literal_operand_type(program, rooted, declared)?,
                            &peeled,
                            false,
                        )?;
                        if projections.len() != 1 {
                            return None;
                        }
                        let projection = projections.remove(0);
                        expression = projection.expression;
                        demanded = projection.remaining;
                        depth -= 1;
                    }
                    _ => return None,
                }
            }
            _ => return None,
        }
    }
}
