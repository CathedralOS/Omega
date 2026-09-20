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
    // progress supplies the two producers it can prove exactly: an owned
    // helper result substitutes the caller argument place the callee's
    // returned expression establishes, and a demanded place that still names
    // storage through a reference leaf inside an owned local rebases to the
    // referent the leaf's latest store supplied at each frontier.
    let mut owned_frames = None;
    let frames = flow::shared_call_frames_or(call_frames, program, &mut owned_frames)?;
    let resolve = |state: &FlowStateFact,
                   statement_index: usize,
                   call: &TableCallExpression,
                   relative: &[PlaceSegment]| {
        call_result_value_place(program, frames, state, statement_index, call, relative, 16)
    };
    // A leaf demanded directly as the call's operand through an exclusive
    // binding — `r.view.scheduler` where `r: &mut RefBox` — cannot run the
    // operand-prefix check on its literal spelling: the shared prefix walk
    // keeps no origin for an exclusive borrow of a carrier that itself stores
    // shared leaves. Resolve the referent from the binding's own provenance
    // first so the ordinary reference-storage check sees the spelling it can
    // name.
    let place = exclusive_reference_root_place(
        program,
        frames,
        machine,
        state,
        call.statement_index,
        &place,
        &resolve,
    )
    .unwrap_or(place);
    let place = flow::value_origin_at_call_resolving(
        program,
        flow,
        machine,
        state,
        call,
        place,
        Some(frames),
        &resolve,
        |state, bound, place| {
            reference_boundary_before_statement(
                program, frames, machine, state, bound, place, &resolve,
            )
        },
    )?;
    crate::checks::termination::progress::fact_subjects::subject_from_place(
        place.root,
        &place.segments,
    )
}

/// The exact caller-side place a premise-bearing call argument names. A
/// spelled place keeps its own identity; an argument that is itself a nested
/// call result — or a projection peeled off one, or a constructor operand —
/// has no storage of its own, so the same `caller_argument_place` replay that
/// resolves stored helper results proves which input supplied it. Anything
/// the replay cannot prove stays unproven rather than borrowing a same-shaped
/// root.
pub(super) fn call_argument_place(
    program: &TypedTrees,
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

/// How many reference-boundary hops one demanded place may compose before a
/// cyclic spelling is refused. Each hop resolves one authored boundary — a
/// reference-typed binding, a write-capable leaf's stored origin, or a shared
/// leaf's supplying store — so the bound matches the call-result recursion
/// depth.
const REFERENCE_BOUNDARY_HOPS: usize = 16;

/// The per-frontier rebase the shared trace applies to each demanded place.
/// `rebase_exact_local_place` covers a write-capable leaf carried inside an
/// owned local through the prefix's stored origins; a shared leaf has no
/// write origins — nothing may write through it — so
/// `shared_reference_leaf_origin` resolves the store that supplied its
/// referent instead. `bound` is the frontier: one past the statement the scan
/// is examining, so stores at that statement count. An unresolved hop keeps
/// the literal place, which the scan then proves or refuses under its
/// ordinary rules; a resolved place may still cross another boundary, which
/// the next frontier's rebase — or the leaf scan's own relocation — resolves.
fn reference_boundary_before_statement<Resolve>(
    program: &TypedTrees,
    frames: &validation::CallFrameResolver<'_>,
    machine: &Machine,
    state: &FlowStateFact,
    bound: usize,
    place: &CanonicalPlace,
    resolve: &Resolve,
) -> Option<CanonicalPlace>
where
    Resolve:
        Fn(&FlowStateFact, usize, &TableCallExpression, &[PlaceSegment]) -> Option<CanonicalPlace>,
{
    let mut next = flow::rebase_exact_local_place(
        program,
        state.state_symbol,
        bound,
        place.clone(),
        Some(frames),
    )
    .unwrap_or_else(|| place.clone());
    let exclusive =
        exclusive_reference_root_place(program, frames, machine, state, bound, &next, resolve);
    if let Some(resolved) = exclusive {
        next = resolved;
    }
    if let Some(resolved) =
        shared_reference_leaf_origin(program, frames, machine, state, bound, &next, resolve)
    {
        next = resolved;
    }
    (next != *place).then_some(next)
}

/// A demanded place spelled through an exclusive-binding root —
/// `r.view.scheduler` where `r: &mut RefBox` — names the referent the
/// binding's latest supply proved, but the shared prefix walk cannot always
/// name that referent: an exclusive borrow of a carrier that itself stores
/// shared leaves keeps no alias-table origin, leaving the operand's literal
/// root without a resolvable spelling. This scan replays the binding's own
/// provenance instead: the declaration's operand, then every later bare-name
/// store `r = ..`, each canonicalized the way a stored operand would be — a
/// borrow's target, another binding's name, or a checked call result the
/// resolver proves. A statement between the supply and `bound` whose writes
/// may touch the bare binding — a store the frame cannot name, an operand
/// call's frame, a `r.field` write reachable through the referent — fails
/// closed rather than guessing the referent survived. The referent itself
/// may ride through further reference bindings; each hop repeats the scan at
/// the position where it was captured.
fn exclusive_reference_root_place<Resolve>(
    program: &TypedTrees,
    frames: &validation::CallFrameResolver<'_>,
    machine: &Machine,
    state: &FlowStateFact,
    bound: usize,
    place: &CanonicalPlace,
    resolve: &Resolve,
) -> Option<CanonicalPlace>
where
    Resolve:
        Fn(&FlowStateFact, usize, &TableCallExpression, &[PlaceSegment]) -> Option<CanonicalPlace>,
{
    let typed_state = crate::semantic_calls::find_state(program, state.state_symbol)?;
    let statements = program
        .statement_table
        .statements(typed_state.statement_nodes);
    let mut demanded = place.clone();
    let mut cursor = bound;
    for _ in 0..REFERENCE_BOUNDARY_HOPS {
        let PlaceRoot::Symbol(root) = demanded.root else {
            return Some(demanded);
        };
        let Some((decl_index, local)) =
            statements
                .get(..cursor)?
                .iter()
                .enumerate()
                .find_map(|(index, statement)| match statement {
                    StatementNode::LocalData(local) if local.symbol == root => Some((index, local)),
                    _ => None,
                })
        else {
            // A parameter or member root keeps its access path.
            return Some(demanded);
        };
        match reference_type(program, local.type_reference) {
            None => return Some(demanded),
            Some(language_semantics::ReferenceAccess::Shared) => {
                // A shared binding is the ordinary reference query's subject;
                // when it cannot name the referent the demand stays unproven
                // rather than minting one from this replay.
                demanded = flow::local_reference_storage_before_statement(
                    program, frames, machine, state, cursor, demanded,
                )?;
                continue;
            }
            _ => {}
        }
        // `slot` is the binding's own storage: only a bare-name assignment
        // may replace it, and any other overlap is an untracked touch.
        let slot = CanonicalPlace {
            root: PlaceRoot::Symbol(root),
            segments: Vec::new(),
        };
        let mut referent = match reference_bound_operand_place(
            program,
            frames,
            machine,
            state,
            decl_index,
            local.initial_value,
            resolve,
        ) {
            Some(place) => place,
            None => return None,
        };
        let mut captured = decl_index;
        for (index, statement) in statements
            .get(decl_index + 1..cursor)?
            .iter()
            .enumerate()
            .map(|(offset, statement)| (decl_index + 1 + offset, statement))
        {
            // Operand calls embedded in any statement may reach the binding
            // before its store replays; an unnamed frame fails the hop.
            let writes = flow::frame_storage_writes(
                program,
                machine.symbol,
                state.state_symbol,
                index,
                &frames.statement_value_write_frame(machine, statement),
                Some(frames),
            )?;
            if writes
                .iter()
                .any(|write| places_may_overlap(program, &slot, write))
            {
                return None;
            }
            if let StatementNode::Assignment(assignment) = statement {
                let target = flow::statement_mutated_place(
                    program,
                    machine.symbol,
                    state.state_symbol,
                    index,
                    statement,
                )?;
                if target.root == slot.root && target.segments.is_empty() {
                    // A bare-name store into a reference local either rebinds
                    // it — the referent becomes whatever the store's operand
                    // proves at this position — or writes through it, which
                    // the delegated scans cannot match on this spelling.
                    if frames.assignment_replaces_local_reference_binding(machine, statement)? {
                        referent = reference_bound_operand_place(
                            program,
                            frames,
                            machine,
                            state,
                            index,
                            assignment.value,
                            resolve,
                        )?;
                        captured = index;
                        continue;
                    }
                    return None;
                }
                if places_may_overlap(program, &slot, &target) {
                    return None;
                }
            }
            let writes = flow::statement_storage_writes(
                program,
                machine.symbol,
                state.state_symbol,
                index,
                statement,
                Some(frames),
            )?;
            if writes
                .iter()
                .any(|write| places_may_overlap(program, &slot, write))
            {
                return None;
            }
            if let StatementNode::Call(call_statement) = statement {
                let writes = flow::frame_storage_writes(
                    program,
                    machine.symbol,
                    state.state_symbol,
                    index,
                    &frames.may_write_frame(machine, call_statement),
                    Some(frames),
                )?;
                if writes
                    .iter()
                    .any(|write| places_may_overlap(program, &slot, write))
                {
                    return None;
                }
            }
        }
        let mut rebased = referent;
        rebased.segments.extend_from_slice(&demanded.segments);
        demanded = rebased;
        cursor = captured;
    }
    None
}

/// The place a reference-binding operand supplies: a borrow's canonicalized
/// target or another binding's name, and a result-position call asks the
/// domain resolver for the caller place its checked callee proves — the same
/// leaf-store discipline `leaf_value_candidate` applies to assignment
/// operands.
fn reference_bound_operand_place<Resolve>(
    program: &TypedTrees,
    frames: &validation::CallFrameResolver<'_>,
    machine: &Machine,
    state: &FlowStateFact,
    index: usize,
    value: ExpressionHandle,
    resolve: &Resolve,
) -> Option<CanonicalPlace>
where
    Resolve:
        Fn(&FlowStateFact, usize, &TableCallExpression, &[PlaceSegment]) -> Option<CanonicalPlace>,
{
    let candidate =
        flow::canonical_place_from_expression_in_state(program, state.state_symbol, index, value)?;
    match candidate.root {
        PlaceRoot::Symbol(root) => {
            // The operand must name storage this state owns — a local or
            // parameter declared under it, or the machine's own attached
            // data — never a same-spelled symbol living in another body.
            if !(root == machine.symbol || program.symbols.get(root).parent == state.state_symbol) {
                return None;
            }
            // When the operand's own path ends in a reference leaf —
            // `saved.context` supplying `let borrowed: &mut Context` — the
            // binding's referent is the leaf's pointee, not the slot. A
            // shared leaf's referent is its stored origin via the leaf scan;
            // a write-capable leaf's is the prefix's exact stored origin.
            // Provenance that stays unproven keeps the whole operand unproven.
            let Some(root_type) =
                statements_local_type(program, state, root, index).or_else(|| {
                    crate::semantic_calls::find_state(program, state.state_symbol).and_then(
                        |typed_state| {
                            program
                                .state_parameters(typed_state)
                                .iter()
                                .find(|parameter| parameter.symbol == root)
                                .map(|parameter| parameter.type_reference)
                        },
                    )
                })
            else {
                return None;
            };
            let reached = flow::project_type_reference_from_segments(
                program,
                root_type,
                &candidate.segments,
            )?;
            match reference_type(program, reached) {
                Some(language_semantics::ReferenceAccess::Shared) => shared_reference_leaf_origin(
                    program, frames, machine, state, index, &candidate, resolve,
                ),
                Some(_) => flow::rebase_exact_local_place(
                    program,
                    state.state_symbol,
                    index,
                    candidate,
                    Some(frames),
                ),
                None => Some(candidate),
            }
        }
        PlaceRoot::Expression(rooted) => match program.expression_table.expression(rooted) {
            ExpressionNode::Call(call) => resolve(state, index, call, &candidate.segments),
            _ => None,
        },
        _ => None,
    }
}

/// The declared type of the `LocalData` declaring `root` before `bound`, when
/// `root` is a local at all — parameters and members have no declaration
/// statement.
fn statements_local_type(
    program: &TypedTrees,
    state: &FlowStateFact,
    root: symbols::SymbolHandle,
    bound: usize,
) -> Option<TypeReferenceHandle> {
    let typed_state = crate::semantic_calls::find_state(program, state.state_symbol)?;
    program
        .statement_table
        .statements(typed_state.statement_nodes)
        .get(..bound)?
        .iter()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.symbol == root => Some(local.type_reference),
            _ => None,
        })
}

/// What one resolved store did to the leaf slot under the scan.
enum LeafArrival {
    /// The slot's contents trace to this exact storage — the referent for a
    /// shared leaf, or the caller-side place a proven call result carried.
    Resolved(CanonicalPlace),
    /// The slot's contents were copied out of another shared-reference slot;
    /// the scan continues with that slot's earlier writes at the same point
    /// in time.
    Relocate(CanonicalPlace),
}

/// Resolve the exact referent a shared-reference leaf inside an owned local
/// holds at `call`, when the demanded place crosses one. `place` names the
/// demanded storage; its leading `leaf` segments select a `&`-typed field of
/// a local carrier — `boxed.view` inside `boxed.view.scheduler`. Nothing can
/// write through a shared reference, so the only writes that matter are the
/// ones rebinding the slot itself or replacing an enclosing slot: each is the
/// store that supplied the referent. The scan walks backward to the latest
/// such store — an assignment target equal to the slot (rebind), an ancestor
/// of it (carrier replacement), or the local's own declaration — and resolves
/// the stored operand's referent. Every other overlapping write — a call
/// frame, an operand call embedded in any statement, a through-leaf or
/// alias-closed store — fails closed. A leaf value copied out of another
/// carrier's leaf relocates the slot and keeps scanning that slot's earlier
/// writes, so the referent is the one held at the copy, not the one the
/// source slot holds at the call.
fn shared_reference_leaf_origin<Resolve>(
    program: &TypedTrees,
    frames: &validation::CallFrameResolver<'_>,
    machine: &Machine,
    state: &FlowStateFact,
    bound: usize,
    place: &CanonicalPlace,
    resolve: &Resolve,
) -> Option<CanonicalPlace>
where
    Resolve:
        Fn(&FlowStateFact, usize, &TableCallExpression, &[PlaceSegment]) -> Option<CanonicalPlace>,
{
    let typed_state = crate::semantic_calls::find_state(program, state.state_symbol)?;
    let statements = program
        .statement_table
        .statements(typed_state.statement_nodes);
    let PlaceRoot::Symbol(root) = place.root else {
        return None;
    };
    // Only an owned local's declaration supplies the boundary's frame type; a
    // parameter or member root keeps its access path unchanged.
    let local = statements
        .get(..bound)?
        .iter()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.symbol == root => Some(local),
            _ => None,
        })?;
    let leaf_len = shared_reference_boundary(program, local.type_reference, &place.segments)?;
    let mut slot = CanonicalPlace {
        root: place.root,
        segments: place.segments[..leaf_len].to_vec(),
    };
    let suffix = &place.segments[leaf_len..];
    for (index, statement) in statements.get(..bound)?.iter().enumerate().rev() {
        // Operand calls embedded in any statement — including the supplying
        // store itself — run first; a frame that may touch the slot keeps the
        // leaf's contents unproven.
        let writes = flow::frame_storage_writes(
            program,
            machine.symbol,
            state.state_symbol,
            index,
            &frames.statement_value_write_frame(machine, statement),
            Some(frames),
        )?;
        if writes
            .iter()
            .any(|write| places_may_overlap(program, &slot, write))
        {
            return None;
        }
        let arrival = match statement {
            StatementNode::LocalData(local) if slot.root == PlaceRoot::Symbol(local.symbol) => {
                // The carrier's own declaration supplied the leaf: the
                // initializer's operand at the leaf path is the stored value.
                let candidate = leaf_value_candidate(
                    program,
                    state,
                    index,
                    local.initial_value,
                    local.type_reference,
                    &slot.segments,
                    resolve,
                )?;
                Some(leaf_candidate_arrival(
                    program, frames, machine, state, index, candidate,
                )?)
            }
            StatementNode::Assignment(assignment) => {
                let target = flow::statement_mutated_place(
                    program,
                    machine.symbol,
                    state.state_symbol,
                    index,
                    statement,
                )?;
                let target = flow::local_reference_storage_before_statement(
                    program, frames, machine, state, index, target,
                )?;
                let target_is_slot = target.root == slot.root;
                let stored_type = || {
                    validation::declared_place_type_raw(
                        program,
                        machine,
                        Some(typed_state),
                        assignment.target,
                    )
                };
                if target_is_slot && target.segments == slot.segments {
                    // A store into exactly this slot rebinds the leaf: the
                    // right-hand value is the new referent.
                    let candidate = leaf_value_candidate(
                        program,
                        state,
                        index,
                        assignment.value,
                        stored_type()?,
                        &[],
                        resolve,
                    )?;
                    Some(leaf_candidate_arrival(
                        program, frames, machine, state, index, candidate,
                    )?)
                } else if target_is_slot && slot.segments.starts_with(&target.segments) {
                    // A store into an enclosing slot replaces the carrier; the
                    // leaf's contents are the projection of the stored value
                    // at the remaining leaf path.
                    let candidate = leaf_value_candidate(
                        program,
                        state,
                        index,
                        assignment.value,
                        stored_type()?,
                        &slot.segments[target.segments.len()..],
                        resolve,
                    )?;
                    Some(leaf_candidate_arrival(
                        program, frames, machine, state, index, candidate,
                    )?)
                } else {
                    let writes = flow::statement_storage_writes(
                        program,
                        machine.symbol,
                        state.state_symbol,
                        index,
                        statement,
                        Some(frames),
                    )?;
                    if writes
                        .iter()
                        .any(|write| places_may_overlap(program, &slot, write))
                    {
                        return None;
                    }
                    None
                }
            }
            StatementNode::Call(call_statement) => {
                let writes = flow::frame_storage_writes(
                    program,
                    machine.symbol,
                    state.state_symbol,
                    index,
                    &frames.may_write_frame(machine, call_statement),
                    Some(frames),
                )?;
                if writes
                    .iter()
                    .any(|write| places_may_overlap(program, &slot, write))
                {
                    return None;
                }
                None
            }
            _ => {
                let writes = flow::statement_storage_writes(
                    program,
                    machine.symbol,
                    state.state_symbol,
                    index,
                    statement,
                    Some(frames),
                )?;
                if writes
                    .iter()
                    .any(|write| places_may_overlap(program, &slot, write))
                {
                    return None;
                }
                None
            }
        };
        match arrival {
            Some(LeafArrival::Resolved(mut resolved)) => {
                resolved.segments.extend_from_slice(suffix);
                return Some(resolved);
            }
            Some(LeafArrival::Relocate(next)) => slot = next,
            None => {}
        }
    }
    // The prefix never stored the slot: it names its root's entry value. A
    // parameter or member root keeps the access path, and the delegated scan
    // still proves or refuses it under its ordinary rules — including any
    // later rebind the relocated slot endured before the call.
    let mut resolved = slot;
    resolved.segments.extend_from_slice(suffix);
    Some(resolved)
}

/// The first `&`-shared boundary in a demanded path: the number of leading
/// segments whose projection through the root's declared type lands on a
/// shared reference. An exclusive boundary is not this scan's subject —
/// write-capable leaves rebase through the prefix's stored origins — and an
/// unresolvable projection leaves the literal place for the delegated scan.
fn shared_reference_boundary(
    program: &TypedTrees,
    root_type: TypeReferenceHandle,
    segments: &[PlaceSegment],
) -> Option<usize> {
    for boundary in 1..=segments.len() {
        let reached =
            flow::project_type_reference_from_segments(program, root_type, &segments[..boundary])?;
        if let Some(access) = reference_type(program, reached) {
            return (!access.is_exclusive()).then_some(boundary);
        }
    }
    None
}

/// The operand that supplied a stored leaf: `wanted` projects the stored
/// `value` — a declaration initializer or an assignment's right-hand side —
/// down to the expression bound for the leaf, which canonicalizes to a place.
/// A result-position call instead asks the domain resolver for the caller
/// place its checked callee proves; every other non-storage expression —
/// conditional routes, opaque projections — is refused rather than borrowed
/// as a same-shaped guess.
fn leaf_value_candidate<Resolve>(
    program: &TypedTrees,
    state: &FlowStateFact,
    index: usize,
    value: ExpressionHandle,
    declared_type: TypeReferenceHandle,
    wanted: &[PlaceSegment],
    resolve: &Resolve,
) -> Option<CanonicalPlace>
where
    Resolve:
        Fn(&FlowStateFact, usize, &TableCallExpression, &[PlaceSegment]) -> Option<CanonicalPlace>,
{
    let mut projections =
        flow::literal_value_projections(program, value, declared_type, wanted, false)?;
    if projections.len() != 1 {
        return None;
    }
    let projection = projections.remove(0);
    let mut candidate = flow::canonical_place_from_expression_in_state(
        program,
        state.state_symbol,
        index,
        projection.expression,
    )?;
    candidate.segments.extend_from_slice(&projection.remaining);
    match candidate.root {
        PlaceRoot::Symbol(_) => Some(candidate),
        PlaceRoot::Expression(rooted) => match program.expression_table.expression(rooted) {
            ExpressionNode::Call(call) => resolve(state, index, call, &candidate.segments),
            _ => None,
        },
        _ => None,
    }
}

/// Classify the place a stored leaf operand arrived at. A candidate whose
/// root names no local in this prefix — a parameter or machine member — keeps
/// its access path. A reference-typed local names its referent rather than
/// its slot, so it rebases through the reference query at this exact prefix
/// position and classifies again. An owned local whose projected path still
/// lands on a shared reference is another carrier's leaf slot: the scan
/// relocates to it. Anything else is the referent itself.
fn leaf_candidate_arrival(
    program: &TypedTrees,
    frames: &validation::CallFrameResolver<'_>,
    machine: &Machine,
    state: &FlowStateFact,
    index: usize,
    mut candidate: CanonicalPlace,
) -> Option<LeafArrival> {
    let typed_state = crate::semantic_calls::find_state(program, state.state_symbol)?;
    let statements = program
        .statement_table
        .statements(typed_state.statement_nodes);
    for _ in 0..REFERENCE_BOUNDARY_HOPS {
        let PlaceRoot::Symbol(root) = candidate.root else {
            return None;
        };
        let Some(local) = statements
            .get(..index)?
            .iter()
            .find_map(|statement| match statement {
                StatementNode::LocalData(local) if local.symbol == root => Some(local),
                _ => None,
            })
        else {
            return Some(LeafArrival::Resolved(candidate));
        };
        if reference_type(program, local.type_reference).is_some() {
            // The candidate names storage through this binding, not the
            // binding itself: rebase to its referent at this prefix position,
            // then classify that place.
            candidate = flow::local_reference_storage_before_statement(
                program, frames, machine, state, index, candidate,
            )?;
            continue;
        }
        let reached = flow::project_type_reference_from_segments(
            program,
            local.type_reference,
            &candidate.segments,
        )?;
        return Some(match reference_type(program, reached) {
            Some(language_semantics::ReferenceAccess::Shared) => LeafArrival::Relocate(candidate),
            _ => LeafArrival::Resolved(candidate),
        });
    }
    None
}

/// The reference access a type reference names after peeling `Constrained`
/// shells, or none when it names no reference at all.
fn reference_type(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
) -> Option<language_semantics::ReferenceAccess> {
    loop {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Reference { access, .. } => return Some(*access),
            _ => return None,
        }
    }
}
