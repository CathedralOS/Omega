//! Selected-arm payload copies are completed values before target operands.
//! The saved guard receiver supplies the exact address/loan correspondence;
//! each copy retains the selected entry facts and its own whole carrier.
//! Later storage mutation cannot replace that value. Reference descendants
//! still have ordinary live referent epochs, not frozen referee contents.

use arena::Handle;
use checked_trees::{FlowOperandFact, FlowPermissionEvaluation};
use typed_trees::expression::ExpressionHandle;

/// The selected target's completed copy actually consumed by this occurrence.
/// Readable reference crossings are geometry here, not snapshots of referees.
/// The flow operand/observation graph independently authenticates the use;
/// source spelling alone never redirects a guard or an address-forming loan.
pub(crate) fn copied_payload_at<'facts>(
    program: &dyn validation::CheckingInput,
    flow: &'facts checked_trees::FlowFacts,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: usize,
    evaluation: FlowPermissionEvaluation,
    expression: ExpressionHandle,
) -> Option<(Handle<FlowOperandFact>, &'facts FlowOperandFact)> {
    let FlowPermissionEvaluation::TransitionTarget(target) = evaluation else {
        return None;
    };
    let place = validation::canonical_place_from_expression_in_state(
        program, state, statement, expression,
    )?;
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
            if !validation::DispatchExtraction::reads_payload_binding(
                program,
                expression,
                copy.expression,
            ) {
                return None;
            }
            let prefix = validation::canonical_place_from_expression_in_state(
                program,
                state,
                statement,
                copy.expression,
            )?;
            if prefix.root != place.root
                || prefix.segments.len() > place.segments.len()
                || !prefix
                    .segments
                    .iter()
                    .zip(&place.segments)
                    .all(|(left, right)| {
                        super::super::canonical_place_segments_equal(program, *left, *right)
                    })
            {
                return None;
            }
            validation::project_type_reference_from_segments(
                program,
                copy.type_reference,
                &place.segments[prefix.segments.len()..],
            )?;
            let checked_trees::FlowOperandCompletion::Copied { copy: identity, .. } =
                copy.completion
            else {
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
    program: &dyn validation::CheckingInput,
    control: &checked_trees::FlowControlFacts,
    copies: arena::HandleSpan<FlowOperandFact>,
    state: symbols::SymbolHandle,
    statement: usize,
    expression: ExpressionHandle,
    place: &validation::CanonicalPlace,
) -> Option<validation::CanonicalPlace> {
    let mut matching = Vec::new();
    for copy in control.operands.span_or_empty(copies) {
        if !validation::DispatchExtraction::reads_payload_binding(
            program,
            expression,
            copy.expression,
        ) {
            continue;
        }
        let prefix = validation::canonical_place_from_expression_in_state(
            program,
            state,
            statement,
            copy.expression,
        )?;
        if prefix.root != place.root
            || prefix.segments.len() > place.segments.len()
            || !prefix
                .segments
                .iter()
                .zip(&place.segments)
                .all(|(left, right)| {
                    super::super::canonical_place_segments_equal(program, *left, *right)
                })
        {
            continue;
        }
        let path = &place.segments[prefix.segments.len()..];
        let checked_trees::FlowOperandCompletion::Copied { copy: identity, .. } = copy.completion
        else {
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
            let place = validation::CanonicalPlace {
                root: referent.root,
                segments: control
                    .operand_referent_segments
                    .span_or_empty(referent.segments)
                    .to_vec(),
            };
            super::super::project_captured_view_place(place, &path[leaf.path.count() as usize..])?
        } else {
            let projected =
                validation::owned_projection_type_reference(program, copy.type_reference, path)?;
            if super::super::operand_binds_live_view(program, projected) {
                return None;
            }
            validation::CanonicalPlace {
                root: facts::PlaceRoot::Expression(copy.expression),
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
