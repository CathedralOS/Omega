//! Selected-arm payload copies are completed values before target operands.
//! The saved guard receiver supplies the exact address/loan correspondence;
//! each copy retains the selected entry facts and its own whole carrier.
//! Later storage mutation cannot replace that value. Reference descendants
//! still have ordinary live referent epochs, not frozen referee contents.

use super::super::canonical_place_segments_equal;
use super::super::operand_binds_live_view;
use super::super::project_captured_view_place;
use crate::places::place_at;
use arena::{Handle, HandleSpan};
use checked_trees::{
    FlowControlFacts, FlowFacts, FlowOperandCompletion, FlowOperandFact, FlowPermissionEvaluation,
};
use facts::PlaceRoot;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use validation::CanonicalPlace;
use validation::CheckingInput;
use validation::DispatchExtraction;
use validation::owned_projection_type_reference;
use validation::project_type_reference_from_segments;

/// The selected target's completed copy actually consumed by this occurrence.
/// Readable reference crossings are geometry here, not snapshots of referees.
/// The flow operand/observation graph independently authenticates the use;
/// source spelling alone never redirects a guard or an address-forming loan.
pub(crate) fn copied_payload_at<'facts>(
    program: &dyn CheckingInput,
    flow: &'facts FlowFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: usize,
    evaluation: FlowPermissionEvaluation,
    expression: ExpressionHandle,
) -> Option<(Handle<FlowOperandFact>, &'facts FlowOperandFact)> {
    let FlowPermissionEvaluation::TransitionTarget(target) = evaluation else {
        return None;
    };
    let place = place_at(program, state, statement, expression)?;
    let state_flow = flow.control.unique_state(machine, state)?;
    let mut matching = flow
        .control
        .outgoing
        .span_or_empty(state_flow.outgoing)
        .iter()
        .filter(|outgoing| {
            outgoing.statement_index == statement && outgoing.transition_target == target
        })
        .flat_map(|outgoing| outgoing.copied_payloads.handles())
        .filter_map(|handle| {
            let copy = flow.control.operands.get(handle);
            if !DispatchExtraction::reads_payload_binding(program, expression, copy.expression) {
                return None;
            }
            let prefix = place_at(program, state, statement, copy.expression)?;
            if prefix.root != place.root
                || prefix.segments.len() > place.segments.len()
                || !prefix
                    .segments
                    .iter()
                    .zip(&place.segments)
                    .all(|(left, right)| canonical_place_segments_equal(program, *left, *right))
            {
                return None;
            }
            project_type_reference_from_segments(
                program,
                copy.type_reference,
                &place.segments[prefix.segments.len()..],
            )?;
            let FlowOperandCompletion::Copied { copy: identity, .. } = copy.completion else {
                return None;
            };
            if !flow
                .control
                .copied_value_uses
                .span_or_empty(flow.control.copied_values.get(identity).uses)
                .iter()
                .any(|usage| usage.expression == expression && usage.evaluation == evaluation)
            {
                return None;
            }
            Some((handle, copy))
        });
    let found = matching.next()?;
    matching.next().is_none().then_some(found)
}

/// Resolve a value read within an active selected-copy scope. An unmatched
/// place keeps its ordinary identity; an unresolved copied reference never
/// falls back to a newer descriptor at the old source coordinate.
pub(in crate::flow) fn completed_copy_read_place(
    program: &dyn CheckingInput,
    control: &FlowControlFacts,
    copies: HandleSpan<FlowOperandFact>,
    state: SymbolHandle,
    statement: usize,
    expression: ExpressionHandle,
    place: &CanonicalPlace,
) -> Option<CanonicalPlace> {
    let mut matching = Vec::new();
    for copy in control.operands.span_or_empty(copies) {
        if !DispatchExtraction::reads_payload_binding(program, expression, copy.expression) {
            continue;
        }
        let prefix = place_at(program, state, statement, copy.expression)?;
        if prefix.root != place.root
            || prefix.segments.len() > place.segments.len()
            || !prefix
                .segments
                .iter()
                .zip(&place.segments)
                .all(|(left, right)| canonical_place_segments_equal(program, *left, *right))
        {
            continue;
        }
        let path = &place.segments[prefix.segments.len()..];
        let FlowOperandCompletion::Copied { copy: identity, .. } = copy.completion else {
            return None;
        };
        let record = control.copied_values.get(identity);
        let mut leaves = control
            .captured_reference_leaves
            .span_or_empty(record.references)
            .iter()
            .filter(|leaf| path.starts_with(control.operand_child_paths.span_or_empty(leaf.path)));
        let projected = if let Some(leaf) = leaves.next() {
            if leaves.next().is_some() {
                return None;
            }
            let [referent] = control.operand_referents.span_or_empty(leaf.referents) else {
                return None;
            };
            let place = CanonicalPlace {
                root: referent.root,
                segments: control
                    .operand_referent_segments
                    .span_or_empty(referent.segments)
                    .to_vec(),
            };
            project_captured_view_place(place, &path[leaf.path.count() as usize..])?
        } else {
            let projected = owned_projection_type_reference(program, copy.type_reference, path)?;
            if operand_binds_live_view(program, projected) {
                return None;
            }
            CanonicalPlace {
                root: PlaceRoot::Expression(copy.expression),
                segments: path.to_vec(),
            }
        };
        matching.push(projected);
    }
    match matching.as_slice() {
        [] => Some(place.clone()),
        [place] => Some(place.clone()),
        _ => None,
    }
}
