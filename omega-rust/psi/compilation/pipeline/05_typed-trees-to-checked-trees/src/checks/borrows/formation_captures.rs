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

struct CapturedArgument {
    call: Handle<checked_trees::FlowCallFact>,
    operand: Handle<FlowOperandFact>,
}

pub(super) fn actual_invocation(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &checked_trees::FlowStateFact,
    source: Handle<FlowOperandFact>,
) -> Result<Option<Handle<checked_trees::FlowCallFact>>, &'static str> {
    let mut selected = None;
    for argument in argument_occurrences(program, facts, state, source)? {
        if selected.is_some_and(|call| call != argument.call) {
            return Err("the saved actual belongs to multiple invocation tuples");
        }
        selected = Some(argument.call);
    }
    Ok(selected)
}

/// A reference saved by an earlier call keeps that call's actual occurrence.
/// Follow evaluated children, stopping at a completed nested call: its inputs
/// belong to its own invocation, not to the later call receiving its result.
fn argument_occurrences(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &checked_trees::FlowStateFact,
    source: Handle<FlowOperandFact>,
) -> Result<Vec<CapturedArgument>, &'static str> {
    let control = &facts.flow.control;
    if !control.operands.is_valid(source) {
        return Err("the argument source has no completed occurrence");
    }
    let captured = control.operands.get(source);
    let mut arguments = Vec::new();
    for call in state.calls.handles() {
        let row = control.calls.get(call);
        if row.statement_index != captured.statement_index {
            continue;
        }
        for operand in row.argument_operands().handles() {
            let values = evaluated_captures(program, facts, operand)?;
            if values.contains(&source) {
                arguments.push(CapturedArgument { call, operand });
            }
        }
    }
    Ok(arguments)
}

pub(super) struct ArgumentFormation {
    pub(super) value: Handle<FlowOperandFact>,
    pub(super) target: Handle<FlowOperandFact>,
    pub(super) storage: bool,
}

pub(super) fn reference_formation(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    value: Handle<FlowOperandFact>,
) -> Result<Option<ArgumentFormation>, &'static str> {
    if !facts.flow.control.operands.is_valid(value) {
        return Err("the reference formation has no completed value");
    }
    let actual = facts.flow.control.operands.get(value);
    let Some(formation) = validation::reference_formation_source(program, actual.expression) else {
        return Ok(None);
    };
    let target = formation_capture(facts, value, formation.source)
        .ok_or("the reference formation lost its exact evaluated receiver")?;
    Ok(Some(ArgumentFormation {
        value,
        target,
        storage: formation.interpretation == validation::ReferenceFormationInterpretation::Declared
            && validation::borrow_selects_reference_storage(
                program,
                formation.source,
                actual.type_reference,
                &[],
            ),
    }))
}

/// Keep the endpoint that was admitted for this actual. Borrowing the pointer
/// variable itself must not become an access through its current pointer.
pub(super) fn argument_formation(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &checked_trees::FlowStateFact,
    source: Handle<FlowOperandFact>,
) -> Result<Option<ArgumentFormation>, &'static str> {
    let mut selected = None;
    let expression = facts.flow.control.operands.get(source).expression;
    for argument in argument_occurrences(program, facts, state, source)? {
        for value in evaluated_captures(program, facts, argument.operand)? {
            let actual = facts.flow.control.operands.get(value);
            if value != source
                && validation::reference_formation_source(program, actual.expression)
                    .is_none_or(|formation| formation.source != expression)
            {
                continue;
            }
            let Some(formation) = reference_formation(program, facts, value)? else {
                continue;
            };
            if value != source && formation.target != source {
                continue;
            }
            if selected
                .as_ref()
                .is_some_and(|old: &ArgumentFormation| old.value != value)
            {
                return Err("the argument source has multiple formed reference occurrences");
            }
            selected = Some(formation);
        }
    }
    Ok(selected)
}

fn evaluated_captures(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    root: Handle<FlowOperandFact>,
) -> Result<Vec<Handle<FlowOperandFact>>, &'static str> {
    let control = &facts.flow.control;
    if !control.operands.is_valid(root) {
        return Err("the actual argument has no evaluated value");
    }
    let owner = control.operands.get(root);
    let mut pending = vec![(root, false)];
    let mut seen = Vec::new();
    let mut active = Vec::new();
    let mut completed = Vec::new();
    while let Some((value, finished)) = pending.pop() {
        if finished {
            active.retain(|current| *current != value);
            completed.push(value);
            continue;
        }
        if completed.contains(&value) {
            continue;
        }
        if active.contains(&value) {
            return Err("the actual argument's evaluation graph is cyclic");
        }
        if !control.operands.is_valid(value) {
            return Err("the actual argument has an unavailable evaluated child");
        }
        let row = control.operands.get(value);
        if row.state_symbol != owner.state_symbol || row.statement_index != owner.statement_index {
            continue;
        }
        seen.push(value);
        active.push(value);
        pending.push((value, true));
        if matches!(
            row.completion,
            checked_trees::FlowOperandCompletion::InvocationResult { .. }
        ) || matches!(
            program.expression_table.expression(row.expression),
            typed_trees::expression::ExpressionNode::Call(_)
        ) {
            continue;
        }
        pending.extend(row.evaluated_inputs.handles().map(|value| (value, false)));
        if row.backing.is_valid() {
            pending.push((row.backing, false));
        }
        match row.completion {
            checked_trees::FlowOperandCompletion::Constructed(children) => pending.extend(
                control
                    .operand_children
                    .span(children)
                    .ok_or("the actual argument has an unavailable child roster")?
                    .iter()
                    .map(|child| (child.value, false)),
            ),
            checked_trees::FlowOperandCompletion::Selected(alternatives) => pending.extend(
                control
                    .operand_alternatives
                    .span(alternatives)
                    .ok_or("the actual argument has an unavailable alternative roster")?
                    .iter()
                    .map(|alternative| (alternative.value, false)),
            ),
            _ => {}
        }
    }
    Ok(seen)
}
