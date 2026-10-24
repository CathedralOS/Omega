//! Follow completed value predecessors, preserving every alternative and epoch.
//!
//! Geometry only expands a selector. Captured children and stored-value links
//! identify the producers; a checked callee body independently identifies call
//! results. Views keep invocation-live contents, while owned leaves retain their
//! completion frontier. None of these relations grants a loan or membership.

use arena::Handle;
use checked_trees::{FlowCallFact, FlowFacts, FlowOperandFact, FlowStateFact};
use facts::PlaceSegment;
use language_semantics::ProgressSubject;
use typed_trees::{expression::ExpressionNode, machine::Machine};
use validation::CanonicalPlace;

use super::call_places::{
    ArgumentScope, call_argument_place, call_result_place, call_result_value_place,
};
use super::reference_bounds::reference_boundary_before_statement;
use crate::flow::{
    self,
    stored_values::{StoredObservation, StoredOperand},
};

pub(super) enum CapturedOrigins {
    Incoming,
    Exact(Vec<ProgressSubject>),
    Unavailable,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn at_call(
    program: &dyn validation::CheckingInput,
    flow: &FlowFacts,
    semantic: &facts::FactPlan,
    machine: &Machine,
    state: &FlowStateFact,
    call: &FlowCallFact,
    place: &CanonicalPlace,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> CapturedOrigins {
    let Some(_) = program.state_by_symbol(state.state_symbol) else {
        return CapturedOrigins::Unavailable;
    };
    let Some((operands, _)) = flow::captured_call_operands(program, flow, state, call) else {
        return CapturedOrigins::Unavailable;
    };
    let Some(epoch) = operands.last() else {
        return CapturedOrigins::Unavailable;
    };
    let observation = validation::canonical_place_type_reference(
        program,
        state.state_symbol,
        call.statement_index,
        place,
    )
    .filter(|reference| validation::reference_shell(program, *reference).is_some())
    .map_or(StoredObservation::Contents, |_| StoredObservation::Binding);
    match flow::stored_values::at_snapshot(
        program,
        &flow.control,
        epoch.storage_snapshot,
        place,
        observation,
    ) {
        None => CapturedOrigins::Incoming,
        Some(StoredOperand::Unavailable) => CapturedOrigins::Unavailable,
        Some(stored @ StoredOperand::Alternatives(_)) => {
            let Some(captures) = stored.captures() else {
                return CapturedOrigins::Unavailable;
            };
            let mut joined = Vec::new();
            for selected in captures {
                if !selected.reference.is_valid() {
                    return CapturedOrigins::Unavailable;
                }
                let Some(subjects) = resolve_captures(
                    program,
                    flow,
                    semantic,
                    machine,
                    state,
                    call,
                    selected.capture,
                    selected.projection,
                    call_frames,
                ) else {
                    return CapturedOrigins::Unavailable;
                };
                for subject in subjects {
                    if !joined.contains(&subject) {
                        joined.push(subject);
                    }
                }
            }
            CapturedOrigins::Exact(joined)
        }
        Some(StoredOperand::Value {
            capture,
            reference,
            projection,
        }) => {
            if !reference.is_valid() {
                return CapturedOrigins::Unavailable;
            }
            match resolve_captures(
                program,
                flow,
                semantic,
                machine,
                state,
                call,
                capture,
                projection,
                call_frames,
            ) {
                Some(subjects) => CapturedOrigins::Exact(subjects),
                None => CapturedOrigins::Unavailable,
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_captures(
    program: &dyn validation::CheckingInput,
    flow: &FlowFacts,
    semantic: &facts::FactPlan,
    machine: &Machine,
    state: &FlowStateFact,
    call: &FlowCallFact,
    capture: Handle<FlowOperandFact>,
    projection: Vec<PlaceSegment>,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<ProgressSubject>> {
    let mut owned_frames = None;
    let frames = flow::shared_call_frames_or(call_frames, program, &mut owned_frames)?;
    let mut pending = vec![(capture, projection, Vec::new())];
    let mut subjects = Vec::new();
    while let Some((handle, projection, mut ancestry)) = pending.pop() {
        if !flow.control.operands.is_valid(handle) {
            return None;
        }
        let capture = flow.control.operands.get(handle);
        if !capture.type_reference.is_valid()
            || !program
                .expression_table
                .expression_is_valid(capture.expression)
            || capture.state_symbol != state.state_symbol
        {
            return None;
        }
        // Refining a selector consumes one dimension before following a value
        // predecessor. It does not revisit a producer or manufacture a capture.
        if let Some(selector) = projection
            .iter()
            .position(|segment| matches!(segment, PlaceSegment::Index { .. }))
        {
            let collection = validation::project_type_reference_from_segments(
                program,
                capture.type_reference,
                &projection[..selector],
            )?;
            let length = validation::fixed_array_type_length(program, collection)?;
            if length == 0 {
                return None;
            }
            for index in 0..length {
                let mut selected = projection.clone();
                selected[selector] = PlaceSegment::FixedIndex { index };
                pending.push((handle, selected, ancestry.clone()));
            }
            continue;
        }
        if ancestry.contains(&handle) {
            return None;
        }
        ancestry.push(handle);
        let selected_type = validation::project_type_reference_from_segments(
            program,
            capture.type_reference,
            &projection,
        )?;
        let live = validation::reference_shell(program, selected_type).is_some()
            || (!projection.is_empty()
                && validation::reference_shell(program, capture.type_reference).is_some());
        let direct = matches!(
            program.expression_table.expression(capture.expression),
            ExpressionNode::Borrow(_) | ExpressionNode::Call(_)
        ) || (!projection.is_empty()
            && validation::reference_shell(program, capture.type_reference).is_some());
        let observation = if validation::reference_shell(program, capture.type_reference).is_some()
        {
            StoredObservation::Binding
        } else {
            StoredObservation::Contents
        };
        if !direct
            && let Some(predecessors) = flow::completed_values::predecessors_in_flow(
                program,
                flow,
                semantic,
                handle,
                &projection,
                observation,
            )
        {
            if predecessors.is_empty() {
                return None;
            }
            for (predecessor, path) in predecessors {
                pending.push((predecessor, path, ancestry.clone()));
            }
            continue;
        }
        for subject in resolve_leaf(
            program,
            flow,
            machine,
            state,
            call,
            capture,
            &projection,
            live,
            frames,
        )? {
            if !subjects.contains(&subject) {
                subjects.push(subject);
            }
        }
    }
    (!subjects.is_empty()).then_some(subjects)
}

#[allow(clippy::too_many_arguments)]
fn resolve_leaf(
    program: &dyn validation::CheckingInput,
    flow: &FlowFacts,
    machine: &Machine,
    state: &FlowStateFact,
    call: &FlowCallFact,
    capture: &FlowOperandFact,
    projection: &[PlaceSegment],
    live: bool,
    frames: &validation::CallFrameResolver<'_>,
) -> Option<Vec<ProgressSubject>> {
    let sources = if let Some(nested) = program.expression_table.call_node(capture.expression) {
        call_result_place(
            program,
            frames,
            ArgumentScope::Caller {
                state,
                statement_index: capture.statement_index,
            },
            nested,
            projection,
            16,
        )?
    } else {
        vec![call_argument_place(
            program,
            state,
            capture.statement_index,
            capture.expression,
            capture.type_reference,
            projection,
            Some(frames),
        )?]
    };
    let resolve = |state: &FlowStateFact,
                   index,
                   nested: &typed_trees::expression::TableCallExpression,
                   relative: &[PlaceSegment]| {
        call_result_value_place(program, frames, state, index, nested, relative, 16)
    };
    let mut subjects = Vec::new();
    for source in sources {
        if !flow::operand_entry_origin_is_stable(program, flow, capture, &source) {
            return None;
        }
        if live {
            // Freeze the descriptor at production, then observe its original
            // backing at invocation. A replaced unresolved prefix stays opaque.
            let source = flow::local_reference_storage_before_statement(
                program,
                frames,
                machine,
                state,
                capture.statement_index,
                source,
            )?;
            let source = reference_boundary_before_statement(
                program,
                frames,
                machine,
                state,
                capture.statement_index,
                &source,
                &resolve,
            )
            .unwrap_or(source);
            if !bindings_preserved(program, flow, state, call, capture, &source) {
                return None;
            }
            for subject in
                super::direct_at_place(program, flow, machine, state, call, source, Some(frames))?
            {
                if !subjects.contains(&subject) {
                    subjects.push(subject);
                }
            }
        } else {
            let source = flow::trace_value_origin_before_statement(
                program,
                machine,
                state,
                capture.statement_index,
                source,
                frames,
                flow::ValueObservation::Contents,
                Some(flow),
                resolve,
                |state, bound, place| {
                    reference_boundary_before_statement(
                        program, frames, machine, state, bound, place, &resolve,
                    )
                },
            )?;
            let subject = crate::checks::termination::progress::fact_subjects::subject_from_place(
                source.root,
                &source.segments,
            )?;
            if !subjects.contains(&subject) {
                subjects.push(subject);
            }
        }
    }
    (!subjects.is_empty()).then_some(subjects)
}

fn bindings_preserved(
    program: &dyn validation::CheckingInput,
    flow: &FlowFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    capture: &FlowOperandFact,
    source: &CanonicalPlace,
) -> bool {
    let Some(typed_state) = program.state_by_symbol(state.state_symbol) else {
        return false;
    };
    let order = program
        .statement_table
        .admitted_execution_order(typed_state.statement_nodes);
    let Some(start) = order.position(capture.statement_index) else {
        return false;
    };
    let Some(end) = order.position(call.statement_index) else {
        return false;
    };
    if start > end {
        return false;
    }
    for index in &order.indices()[start..end] {
        let Some(statement) = flow.state_statement(state, *index) else {
            return false;
        };
        if !flow::binding_origin_is_stable(
            program,
            &flow.control,
            statement.completed_writes,
            statement.completed_unknown_write,
            source,
        ) {
            return false;
        }
    }
    flow::captured_call_operands(program, flow, state, call)
        .and_then(|(operands, _)| operands.last())
        .is_some_and(|epoch| {
            flow::operand_entry_binding_origin_is_stable(program, flow, epoch, source)
        })
}
