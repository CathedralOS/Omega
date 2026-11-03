//! A new view must use the reference actually read, even if its source variable
//! later changes. Follow saved inputs and selected fields to that captured value;
//! its access permission is checked separately.
use arena::Handle;
use checked_trees::{CheckFacts, FlowOperandFact};

pub(super) fn descriptor_capture(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    source: Handle<FlowOperandFact>,
) -> Option<Handle<FlowOperandFact>> {
    let mut capture = source;
    let mut visited = Vec::new();
    while facts.flow.control.operands.is_valid(capture) && !visited.contains(&capture) {
        visited.push(capture);
        let row = facts.flow.control.operands.get(capture);
        if validation::reference_shell(program, row.type_reference).is_some() {
            return Some(capture);
        }
        row.address_projection?;
        capture = row.backing;
    }
    None
}

pub(super) fn formation_capture(
    facts: &CheckFacts,
    formation: Handle<FlowOperandFact>,
    source: typed_trees::expression::ExpressionHandle,
) -> Option<Handle<FlowOperandFact>> {
    let mut pending = vec![formation];
    let mut visited = Vec::new();
    let mut selected = Handle::invalid();
    while let Some(capture) = pending.pop() {
        if visited.contains(&capture) || !facts.flow.control.operands.is_valid(capture) {
            continue;
        }
        visited.push(capture);
        let row = facts.flow.control.operands.get(capture);
        if row.expression == source {
            if selected.is_valid() {
                return None;
            }
            selected = capture;
        }
        pending.extend(row.evaluated_inputs.handles());
        pending.push(row.backing);
    }
    selected.is_valid().then_some(selected)
}

pub(super) enum AddressReceiver {
    Reference {
        capture: Handle<FlowOperandFact>,
        projection: Vec<facts::PlaceSegment>,
    },
    InlineValue,
    Unavailable,
}

pub(super) fn address_receiver(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    source: Handle<FlowOperandFact>,
) -> AddressReceiver {
    let mut capture = source;
    let mut projection = Vec::new();
    let mut visited = Vec::new();
    while facts.flow.control.operands.is_valid(capture) && !visited.contains(&capture) {
        visited.push(capture);
        let row = facts.flow.control.operands.get(capture);
        if validation::reference_shell(program, row.type_reference).is_some() {
            return AddressReceiver::Reference {
                capture,
                projection,
            };
        }
        let Some(path) = row.address_projection else {
            return if row.backing.is_valid() {
                AddressReceiver::Unavailable
            } else {
                AddressReceiver::InlineValue
            };
        };
        let Some(path) = facts.flow.control.operand_child_paths.span(path) else {
            return AddressReceiver::Unavailable;
        };
        let mut complete = path.to_vec();
        complete.extend(projection);
        projection = complete;
        capture = row.backing;
    }
    AddressReceiver::Unavailable
}
