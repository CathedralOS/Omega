//! Backing exclusions follow descriptor values and exact producer associations.
//! Every alternative must resolve; an opaque image has no mutable source home,
//! an input retains its admitted namespace, and an unknown pointer stays unknown.
use arena::Handle;
use checked_trees::{CapturedPlace, CheckFacts, FlowOperandFact, FlowStateFact};
use facts::PlaceSegment;

mod loan_origins;

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
        .collect::<Vec<_>>();
    let mut visited = Vec::new();
    let mut selected = Vec::new();
    while let Some(operand) = pending.pop() {
        if visited.contains(&operand) {
            continue;
        }
        visited.push(operand);
        let capture = control.operands.get(operand);
        if capture.expression == expression {
            selected.push(operand);
        }
        pending.extend(capture.alternatives.handles());
        pending.extend(
            control
                .operand_children
                .span_or_empty(capture.children)
                .iter()
                .map(|child| child.value),
        );
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
    if path.is_empty()
        && capture.alternatives.is_empty()
        && validation::reference_shell(program, capture.type_reference).is_some()
        && control.operand_retentions.is_valid(capture.retention)
    {
        let enclosing = control
            .operand_retentions
            .get(capture.retention)
            .enclosing_storage;
        if enclosing.is_valid() {
            return enclosing_places(program, facts, state, enclosing, active);
        }
    }
    let Some(selected) = validation::completed_storage::reference_captures_at_path(
        program,
        control,
        operand,
        path,
        capture.storage_snapshot,
    ) else {
        // An unchanged incoming value may use its completion-time loan
        // origin. Explicit unavailable storage never reaches this branch.
        return loan_origins::places(program, facts, state, operand, path);
    };
    let selected = match selected.captures() {
        Some(selected) => selected,
        None => {
            return None;
        }
    };
    if selected.is_empty() {
        return None;
    }
    let mut output = Vec::new();
    let mut needs_loan_origins = false;
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
        if control.operand_retentions.is_valid(descriptor.retention) {
            let enclosing = control
                .operand_retentions
                .get(descriptor.retention)
                .enclosing_storage;
            if enclosing.is_valid() {
                for place in enclosing_places(program, facts, state, enclosing, active)? {
                    if !output.contains(&place) {
                        output.push(place);
                    }
                }
                continue;
            }
        }
        if !descriptor.referents.is_empty() {
            for place in captured_referents(facts, descriptor)? {
                if !output.contains(&place) {
                    output.push(place);
                }
            }
            continue;
        }
        let Some(suppliers) = validation::completed_storage::descriptor_suppliers(
            control,
            descriptor.retention,
            &mut Vec::new(),
        ) else {
            needs_loan_origins = true;
            continue;
        };
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
    if needs_loan_origins {
        // Resolve the observed value at its own completion cut. Its producer
        // may predate installation of the local loan; the consumer's later
        // invocation cut could instead see a replaced descriptor. Incomplete
        // supplier alternatives above never recover an older loan.
        for place in loan_origins::places(program, facts, state, operand, path)? {
            if !output.contains(&place) {
                output.push(place);
            }
        }
    }
    Some(output)
}

fn enclosing_places(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    enclosing: Handle<FlowOperandFact>,
    active: &mut Vec<Query>,
) -> Option<Vec<CapturedPlace>> {
    let storage = facts.flow.control.operands.get(enclosing);
    if validation::reference_shell(program, storage.type_reference).is_some()
        || validation::slice_view_shell(program, storage.type_reference, &[]).is_some()
    {
        return places(program, facts, state, enclosing, &[], active);
    }
    // This capture denotes the owned bytes lent at formation, not a
    // descriptor stored in those bytes. Reading reference contents would
    // incorrectly require a scalar source to contain a pointer.
    captured_referents(facts, storage)
}

fn captured_referents(facts: &CheckFacts, capture: &FlowOperandFact) -> Option<Vec<CapturedPlace>> {
    if capture.referents.is_empty() {
        return None;
    }
    let control = &facts.flow.control;
    control
        .operand_referents
        .span(capture.referents)?
        .iter()
        .map(|referent| {
            Some(CapturedPlace {
                incoming_reference: Handle::invalid(),
                root: referent.root,
                segments: control
                    .operand_referent_segments
                    .span(referent.segments)?
                    .to_vec(),
            })
        })
        .collect()
}
