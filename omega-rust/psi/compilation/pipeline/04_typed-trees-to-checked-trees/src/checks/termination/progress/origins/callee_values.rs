//! callee values slice of origins.
use validation::CanonicalPlace;

use super::{call_places::*, reference_bounds::*};

use super::FlowStateFact;
use crate::flow::{self};
use facts::{PlaceRoot, PlaceSegment};
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::StatementNode;
use typed_trees::types::TypeReferenceHandle;

/// A constructor operand's own declared type: the data type a struct literal
/// declares, the callee parameter's type when the whole operand is the
/// literal, or a constant array's declared type. Without one there is no
/// exact operand provenance.
pub(super) fn literal_operand_type(
    program: &dyn ::validation::CheckingInput,
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
pub(super) fn callee_value_place(
    program: &dyn ::validation::CheckingInput,
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
        let mut projections = flow::literal_expression_projections(
            program,
            expression,
            declared_type,
            relative,
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
    callee_value_place_leaf(program, frames, body, expression, relative, depth)
}

/// The non-constructor leaf of `callee_value_place`. `relative` is appended
/// after the expression's own path: for `local.field` under demanded `[x]`
/// the initializer owes `[field, x]`, and a nested call owes the same
/// composed path as its result projection.
pub(super) fn callee_value_place_leaf(
    program: &dyn ::validation::CheckingInput,
    frames: &validation::CallFrameResolver<'_>,
    body: CalleeBody<'_>,
    expression: ExpressionHandle,
    relative: &[PlaceSegment],
    depth: usize,
) -> Option<CanonicalPlace> {
    if depth == 0 {
        return None;
    }
    let place = validation::canonical_place_from_expression(program, expression)?;
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
                    return single_route(call_result_place(
                        program,
                        frames,
                        ArgumentScope::Callee { body },
                        nested,
                        &nested_relative,
                        depth - 1,
                    ));
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
                    let mut projections = flow::literal_expression_projections(
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
    let Some((_, local)) = program
        .state_local_declaration(body.state, root)
        .filter(|&(ordinal, _)| ordinal < body.prefix.len())
    else {
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
        let demanded_place = CanonicalPlace {
            root: PlaceRoot::Symbol(local.symbol),
            segments: demanded.clone(),
        };
        if callee_leaves_demanded_path_unwritten(
            program,
            frames,
            body.machine,
            body.state,
            &demanded_place,
        )
        .is_none()
        {
            // A mutable local's demanded path was written: an exact store
            // supplies the value it names, so replay the place through the
            // body's own prefix rather than the declaration's initializer.
            return callee_demanded_origin(program, frames, body, demanded_place, depth);
        }
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

/// A callee's demanded place replayed through the body's own statement
/// stream: an exact store into the path supplies the value's provenance —
/// the replacement input, not the written binding — and any write the shared
/// backward trace cannot replay leaves the path unproven. Nested call results
/// resolve through the same callee scope at a decremented depth.
pub(super) fn callee_demanded_origin(
    program: &dyn ::validation::CheckingInput,
    frames: &validation::CallFrameResolver<'_>,
    body: CalleeBody<'_>,
    place: CanonicalPlace,
    depth: usize,
) -> Option<CanonicalPlace> {
    if depth == 0 {
        return None;
    }
    // The result is evaluated after the whole prefix, so the trace bound is
    // the terminal statement's index: every statement in `prefix` precedes it.
    let fact = FlowStateFact {
        machine_symbol: body.machine.symbol,
        state_symbol: body.state.symbol,
        ..FlowStateFact::default()
    };
    let resolve = |_: &FlowStateFact,
                   _index: usize,
                   call: &TableCallExpression,
                   relative: &[PlaceSegment]| {
        single_route(call_result_place(
            program,
            frames,
            ArgumentScope::Callee { body },
            call,
            relative,
            depth - 1,
        ))
    };
    flow::trace_value_origin_before_statement(
        program,
        body.machine,
        &fact,
        body.prefix.len(),
        place,
        frames,
        flow::ValueObservation::Contents,
        None,
        resolve,
        |state: &FlowStateFact, bound: usize, place: &CanonicalPlace| {
            reference_boundary_before_statement(
                program,
                frames,
                body.machine,
                state,
                bound,
                place,
                &resolve,
            )
        },
    )
}

/// Whether no statement in this checked body may write the demanded place.
/// Mutable and reference-capable bindings keep exact provenance only while
/// every write — a direct store, a statement-position call, or a call
/// embedded in any operand — provably lands outside the demanded projection.
/// An opaque frame or an overlapping write proves nothing, so the binding
/// stays unproven rather than borrowing a same-shaped guarantee.
pub(super) fn callee_leaves_demanded_path_unwritten(
    program: &dyn ::validation::CheckingInput,
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
