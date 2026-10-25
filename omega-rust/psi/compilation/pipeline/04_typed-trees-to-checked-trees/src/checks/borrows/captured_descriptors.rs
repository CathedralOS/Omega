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
