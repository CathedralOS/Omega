//! Join the invocation's captured operands to their physical backing.
//! The shared completed-storage query owns descriptor/enclosure geometry;
//! this borrow-checking adapter alone supplies checked completion-time loans.
use arena::Handle;
use checked_trees::{CapturedPlace, CheckFacts, FlowOperandFact, FlowStateFact};
use facts::PlaceSegment;

mod loan_origins;

pub(super) fn places(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    operand: Handle<FlowOperandFact>,
    path: &[PlaceSegment],
) -> Option<Vec<CapturedPlace>> {
    validation::completed_storage::reference_backing_places(
        program,
        &facts.flow.control,
        operand,
        path,
        &mut |operand, path| loan_origins::places(program, facts, state, operand, path),
    )
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
        for place in places(program, facts, state, operand, &[])? {
            if !output.contains(&place) {
                output.push(place);
            }
        }
    }
    Some(output)
}
