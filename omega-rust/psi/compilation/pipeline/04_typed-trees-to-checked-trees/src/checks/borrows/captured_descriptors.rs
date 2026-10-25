//! Backing exclusions follow descriptor values and exact producer associations.
//! Every alternative must resolve; an opaque image has no mutable source home,
//! an input retains its admitted namespace, and an unknown pointer stays unknown.
use arena::Handle;
use checked_trees::{CapturedPlace, CheckFacts, FlowOperandFact, FlowStateFact};
use facts::PlaceSegment;

mod incoming;

type Query = (Handle<FlowOperandFact>, Vec<PlaceSegment>);

pub(super) fn places(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    operand: Handle<FlowOperandFact>,
    path: &[PlaceSegment],
    active: &mut Vec<Query>,
) -> Option<Vec<CapturedPlace>> {
    let query = (operand, path.to_vec());
    if active.contains(&query) || !facts.flow.control.operands.is_valid(operand) {
        return None;
    }
    active.push(query);
    let result = resolve(program, facts, state, operand, path, active);
    active.pop();
    result
}

/// Rejoin a forwarded reference read to its exact completed value, including
/// children evaluated inside constructed arguments. A descriptor's former home
/// is not the storage its word points at. Missing capture identities cannot establish a different backing or an empty reference frontier.
pub(super) fn argument_places(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    statement: usize,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<Vec<CapturedPlace>> {
    let control = &facts.flow.control;
    let mut pending = control
        .calls
        .span_or_empty(state.calls)
        .iter()
        .filter(|call| call.statement_index == statement)
        .flat_map(|call| call.operands.handles())
        .map(|operand| (operand, false))
        .collect::<Vec<_>>();
    let mut active = Vec::new();
    let mut finished = Vec::new();
    let mut selected = Vec::new();
    while let Some((operand, leaving)) = pending.pop() {
        if leaving {
            if active.pop() != Some(operand) {
                return None;
            }
            finished.push(operand);
            continue;
        }
        if finished.contains(&operand) {
            continue;
        }
        if active.contains(&operand) || !control.operands.is_valid(operand) {
            return None;
        }
        active.push(operand);
        pending.push((operand, true));
        let capture = control.operands.get(operand);
        if capture.expression == expression {
            selected.push(operand);
        }
        // Only the actual completed graph supplies evaluated occurrences.
        // Selected arms are alternatives; constructed children coexist. A
        // copied endpoint already retains its descriptor in this capture.
        match capture.completion {
            checked_trees::FlowOperandCompletion::Selected(alternatives) => {
                let alternatives = control.operand_alternatives.span(alternatives)?;
                if alternatives.is_empty() {
                    return None;
                }
                pending.extend(alternatives.iter().map(|arm| (arm.value, false)));
            }
            checked_trees::FlowOperandCompletion::Constructed(children) => {
                pending.extend(
                    control
                        .operand_children
                        .span(children)?
                        .iter()
                        .map(|child| (child.value, false)),
                );
            }
            checked_trees::FlowOperandCompletion::Observation
            | checked_trees::FlowOperandCompletion::Copied { .. } => {}
        }
    }
    if selected.is_empty() {
        return None;
    }
    let mut output = Vec::new();
    for operand in selected {
        for place in places(program, facts, state, operand, &[], &mut Vec::new())? {
            if !output.contains(&place) {
                output.push(place);
            }
        }
    }
    Some(output)
}

fn resolve(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    operand: Handle<FlowOperandFact>,
    path: &[PlaceSegment],
    active: &mut Vec<Query>,
) -> Option<Vec<CapturedPlace>> {
    let control = &facts.flow.control;
    let capture = control.operands.get(operand);
    use super::resources::call_operands::FrozenReferenceSelection;
    let frozen =
        super::resources::call_operands::frozen_reference_leaf(program, control, operand, path);
    if matches!(&frozen, FrozenReferenceSelection::Unavailable) {
        return None;
    }
    if let FrozenReferenceSelection::Endpoint {
        owner,
        leaf,
        path: leaf_path,
    } = frozen
    {
        let referents = control.operand_referents.span(leaf.referents)?;
        if referents.is_empty() {
            let profiles = validation::completed_storage::captured_retention_profiles_at_path(
                program, control, capture, path,
            )?;
            return (!profiles.is_empty()
                && profiles.iter().all(|profile| {
                    control.operand_retentions.is_valid(*profile)
                        && control.operand_retentions.get(*profile).immutable_image
                        && control
                            .retained_roots(*profile)
                            .is_some_and(|roots| roots.is_empty())
                }))
            .then_some(Vec::new());
        }
        // Zero formation is the producer's direct-geometry lane. A nonzero
        // handle must authenticate a retained formation before classification.
        if referents.iter().any(|referent| {
            if !referent.formation.is_valid() {
                return false;
            }
            if !control.referent_formations.is_valid(referent.formation) {
                return true;
            }
            let formation = control.referent_formations.get(referent.formation);
            formation.owner.is_valid() && !control.operands.is_valid(formation.owner)
        }) {
            return None;
        }
        let deferred = |referent: &checked_trees::FlowOperandReferent| {
            control.referent_formations.is_valid(referent.formation)
                && control
                    .referent_formations
                    .get(referent.formation)
                    .owner
                    .is_valid()
        };
        if referents.iter().any(deferred) {
            // Producers may mix completed descriptor geometry and deferred
            // ingress inventory. The aggregate ingress query cannot reconstruct
            // that mixed roster, so only an entirely deferred frontier rejoins.
            if !referents.iter().all(deferred) {
                return None;
            }
            return incoming::captured_places(
                program,
                facts,
                state,
                &owner,
                &leaf_path,
                leaf.constraints,
            );
        }
        let mut output = Vec::new();
        for referent in referents {
            let place = CapturedPlace {
                incoming_reference: Handle::invalid(),
                root: referent.root,
                segments: control
                    .operand_referent_segments
                    .span(referent.segments)?
                    .to_vec(),
            };
            if !output.contains(&place) {
                output.push(place);
            }
        }
        return Some(output);
    }
    let Some(selected) = validation::completed_storage::reference_captures_at_path(
        program,
        control,
        operand,
        path,
        capture.storage_snapshot,
    ) else {
        // None alone means unchanged incoming. Unavailable never recovers an
        // old parameter identity or a stale same-slot loan.
        return incoming::places(program, facts, state, operand, path);
    };
    let selected = selected.captures()?;
    if selected.is_empty() {
        return None;
    }
    let mut output = Vec::new();
    for selected in selected {
        if !selected.projection.is_empty() {
            for place in places(
                program,
                facts,
                state,
                selected.capture,
                &selected.projection,
                active,
            )? {
                if !output.contains(&place) {
                    output.push(place);
                }
            }
            continue;
        }
        let descriptor = control.operands.get(selected.capture);
        if control.operand_retentions.is_valid(descriptor.retention)
            && control
                .operand_retentions
                .get(descriptor.retention)
                .immutable_image
            && control
                .retained_roots(descriptor.retention)
                .is_some_and(|roots| roots.is_empty())
        {
            continue;
        }
        if !descriptor.referents.is_empty() {
            for referent in control.operand_referents.span(descriptor.referents)? {
                let place = CapturedPlace {
                    incoming_reference: Handle::invalid(),
                    root: referent.root,
                    segments: control
                        .operand_referent_segments
                        .span(referent.segments)?
                        .to_vec(),
                };
                if !output.contains(&place) {
                    output.push(place);
                }
            }
            continue;
        }
        let suppliers = validation::completed_storage::descriptor_suppliers(
            control,
            descriptor.retention,
            &mut Vec::new(),
        )?;
        if suppliers.is_empty() {
            return None;
        }
        for supplier in suppliers {
            for place in places(
                program,
                facts,
                state,
                supplier.operand,
                control.operand_retained_paths.span(supplier.projection)?,
                active,
            )? {
                if !output.contains(&place) {
                    output.push(place);
                }
            }
        }
    }
    Some(output)
}
