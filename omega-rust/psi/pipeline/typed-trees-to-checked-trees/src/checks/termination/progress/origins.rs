//! Adapt exact value origins to the field-only progress subject surface.

use super::{FlowCallFact, FlowFacts, FlowStateFact, ProgressSubject};
use crate::flow::{self, CanonicalPlace};
use facts::{PlaceRoot, PlaceSegment};
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::state::State;
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
    let mut owned_frames = None;
    let frames = flow::shared_call_frames_or(call_frames, program, &mut owned_frames)?;
    let place = flow::value_origin_at_call_resolving(
        program,
        flow,
        machine,
        state,
        call,
        place,
        Some(frames),
        |state, statement_index, call, relative| {
            call_result_value_place(program, frames, state, statement_index, call, relative, 16)
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
        body: CalleeBody<'a>,
    },
}

/// The checked callee body under proof. The write scan that keeps mutable
/// bindings honest needs the machine/state pair so the shared frame resolver
/// can name this body's may-write places; `prefix` is the transition-free
/// statement run before the returned expression.
#[derive(Clone, Copy)]
struct CalleeBody<'a> {
    machine: &'a Machine,
    state: &'a State,
    prefix: &'a [StatementNode],
}

/// The exact caller-side place an owned call result arrived from, proven only
/// when the callee is a nongeneric checked body whose returned expression
/// resolves to an input projection the body provably leaves unwritten.
/// Receivers, generic or evidence arguments, unresolved dispatch, and opaque
/// write frames stay opaque: a declared result type or a matching spelling is
/// never evidence of a caller identity.
fn call_result_value_place(
    program: &TypedTrees,
    frames: &validation::CallFrameResolver<'_>,
    state: &FlowStateFact,
    statement_index: usize,
    call: &TableCallExpression,
    result_relative: &[PlaceSegment],
    depth: usize,
) -> Option<CanonicalPlace> {
    call_result_place(
        program,
        frames,
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
    frames: &validation::CallFrameResolver<'_>,
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
    let body = CalleeBody {
        machine: callee,
        state: callee_state,
        prefix,
    };
    // `result_relative` is the demanded path into the call result; the callee
    // trace applies it inside its own body so a returned constructor routes
    // the demand to the operand that supplied that exact field or element.
    let returned = callee_value_place(
        program,
        frames,
        body,
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
        // A mutable or write-capable binding still carries exact provenance
        // when no statement in this body may write the demanded projection:
        // writes are what let an owned copy diverge or a reference rebind.
        // Any overlap or opaque frame keeps the result unproven.
        callee_leaves_demanded_path_unwritten(
            program,
            frames,
            callee,
            callee_state,
            &CanonicalPlace {
                root: PlaceRoot::Symbol(parameter.symbol),
                segments: returned.segments.clone(),
            },
        )?;
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
fn scope_argument_place(
    program: &TypedTrees,
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
fn caller_argument_place(
    program: &TypedTrees,
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
        let place = flow::canonical_place_from_expression_in_state(
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
                        return call_result_place(
                            program,
                            frames,
                            ArgumentScope::Caller {
                                state,
                                statement_index,
                            },
                            nested,
                            &peeled,
                            depth,
                        );
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
                        let mut projections = flow::literal_value_projections(
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

/// A constructor operand's own declared type: the data type a struct literal
/// declares, the callee parameter's type when the whole operand is the
/// literal, or a constant array's declared type. Without one there is no
/// exact operand provenance.
fn literal_operand_type(
    program: &TypedTrees,
    rooted: ExpressionHandle,
    declared: Option<TypeReferenceHandle>,
) -> Option<TypeReferenceHandle> {
    match program.expression_table.expression(rooted) {
        ExpressionNode::StructLiteral(literal) => program
            .type_reference_table
            .find_named_type_reference(literal.type_symbol)
            .or(declared),
        ExpressionNode::ArrayLiteral(_) => {
            declared.or_else(|| validation::declared_constant_array_type(program, rooted))
        }
        _ => None,
    }
}

/// The returned expression's exact place in callee space, under the demanded
/// `relative` projection into its value. A parameter or a local traced to its
/// initializer qualifies, as does a nested checked call proven through the
/// same result gate; a mutable binding additionally needs the whole body to
/// provably leave the demanded projection unwritten, and every other root
/// (an opaque expression, an unresolved route) stays unproven.
fn callee_value_place(
    program: &TypedTrees,
    frames: &validation::CallFrameResolver<'_>,
    body: CalleeBody<'_>,
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
            frames,
            body,
            projection.expression,
            &projection.remaining,
            depth - 1,
        );
    }
    callee_value_place_leaf(program, frames, body, expression, relative, depth)
}

/// The non-constructor leaf of `callee_value_place`. `relative` is appended
/// after the expression's own path: for `local.field` under demanded `[x]`
/// the initializer owes `[field, x]`, and a nested call owes the same
/// composed path as its result projection.
fn callee_value_place_leaf(
    program: &TypedTrees,
    frames: &validation::CallFrameResolver<'_>,
    body: CalleeBody<'_>,
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
            match program.expression_table.expression(rooted) {
                // A returned expression that is itself a checked call arrives at
                // that callee's input the same way the outer call arrives at this
                // one's: the proof recurses through the nested callee's own
                // returned expression, keeping the projected segments.
                ExpressionNode::Call(nested) => {
                    let mut nested_relative = place.segments;
                    nested_relative.extend_from_slice(relative);
                    return call_result_place(
                        program,
                        frames,
                        ArgumentScope::Callee { body },
                        nested,
                        &nested_relative,
                        depth - 1,
                    );
                }
                // A member or index peel over a returned constructor selects
                // the operand that supplied the demanded projection, exactly
                // as a bare returned constructor does; that operand then
                // proves its own origin under the same leaf rules.
                ExpressionNode::StructLiteral(_) | ExpressionNode::ArrayLiteral(_) => {
                    let mut peeled = place.segments;
                    peeled.extend_from_slice(relative);
                    if peeled.is_empty() {
                        return None;
                    }
                    let mut projections = flow::literal_value_projections(
                        program,
                        rooted,
                        literal_operand_type(program, rooted, None)?,
                        &peeled,
                        false,
                    )?;
                    if projections.len() != 1 {
                        return None;
                    }
                    let projection = projections.remove(0);
                    return callee_value_place_leaf(
                        program,
                        frames,
                        body,
                        projection.expression,
                        &projection.remaining,
                        depth - 1,
                    );
                }
                _ => return None,
            }
        }
        _ => return None,
    };
    let Some(local) = body.prefix.iter().find_map(|statement| match statement {
        StatementNode::LocalData(local) if local.symbol == root => Some(local),
        _ => None,
    }) else {
        let mut place = place;
        place.segments.extend_from_slice(relative);
        return Some(place);
    };
    // The demanded path into the local's value applies to its initializer,
    // including a constructor bound for later return. A mutable local keeps
    // that correspondence only when no write in the body may overlap it;
    // the scan covers the whole stream, so statements before the declaration
    // simply cannot name the symbol and an untracked alias fails closed.
    let mut demanded = place.segments;
    demanded.extend_from_slice(relative);
    if local.is_mutable {
        callee_leaves_demanded_path_unwritten(
            program,
            frames,
            body.machine,
            body.state,
            &CanonicalPlace {
                root: PlaceRoot::Symbol(local.symbol),
                segments: demanded.clone(),
            },
        )?;
    }
    callee_value_place(
        program,
        frames,
        body,
        local.initial_value,
        local.type_reference,
        &demanded,
        depth - 1,
    )
}

/// Whether no statement in this checked body may write the demanded place.
/// Mutable and reference-capable bindings keep exact provenance only while
/// every write — a direct store, a statement-position call, or a call
/// embedded in any operand — provably lands outside the demanded projection.
/// An opaque frame or an overlapping write proves nothing, so the binding
/// stays unproven rather than borrowing a same-shaped guarantee.
fn callee_leaves_demanded_path_unwritten(
    program: &TypedTrees,
    frames: &validation::CallFrameResolver<'_>,
    callee: &Machine,
    callee_state: &State,
    demanded: &CanonicalPlace,
) -> Option<()> {
    let statements = program
        .statement_table
        .statements(callee_state.statement_nodes);
    for (index, statement) in statements.iter().enumerate() {
        let writes = match statement {
            StatementNode::Assignment(_) | StatementNode::RootBinding(_) => {
                flow::statement_storage_writes(
                    program,
                    callee.symbol,
                    callee_state.symbol,
                    index,
                    statement,
                    Some(frames),
                )?
            }
            StatementNode::Call(call) => flow::frame_storage_writes(
                program,
                callee.symbol,
                callee_state.symbol,
                index,
                &frames.may_write_frame(callee, call),
                Some(frames),
            )?,
            _ => Vec::new(),
        };
        if writes
            .iter()
            .any(|write| places_may_overlap(program, demanded, write))
        {
            return None;
        }
        // Value-position calls nested in any statement — including the
        // returned expression — carry their own operand write frames.
        let writes = flow::frame_storage_writes(
            program,
            callee.symbol,
            callee_state.symbol,
            index,
            &frames.statement_value_write_frame(callee, statement),
            Some(frames),
        )?;
        if writes
            .iter()
            .any(|write| places_may_overlap(program, demanded, write))
        {
            return None;
        }
    }
    Some(())
}

fn places_may_overlap(program: &TypedTrees, left: &CanonicalPlace, right: &CanonicalPlace) -> bool {
    flow::normalized_event_place_root(program, left.root)
        == flow::normalized_event_place_root(program, right.root)
        && flow::canonical_place_segments_may_overlap(program, &left.segments, &right.segments)
}

/// A callee reads a frozen input but cannot change which storage it names:
/// an immutable owned binding or a shared reference whose referent is
/// read-only for the whole call. `&mut`, write-only, and mutable bindings can
/// rebind or rewrite the returned path, so they carry exact caller provenance
/// only when the body's own write scan leaves the demanded projection
/// untouched (`callee_leaves_demanded_path_unwritten`).
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
