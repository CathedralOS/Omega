//! Backing exclusions follow descriptor values and exact producer associations.
//! Every alternative must resolve; an opaque image has no mutable source home,
//! an input retains its admitted namespace, and an unknown pointer stays unknown.
use arena::Handle;
use checked_trees::{CapturedPlace, CheckFacts, FlowOperandFact, FlowStateFact};
use facts::PlaceSegment;

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
    let Some(selected) = validation::completed_storage::reference_captures_at_path(
        program,
        control,
        operand,
        path,
        capture.storage_snapshot,
    ) else {
        // None alone means unchanged incoming. Unavailable never recovers an
        // old parameter identity or a stale same-slot loan.
        return super::incoming::places(program, facts, state, operand, path);
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
