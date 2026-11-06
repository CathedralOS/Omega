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
        if validation::reference_shell(program, row.type_reference).is_some()
            && !row
                .address_projection
                .is_some_and(|projection| projection.forms_view())
        {
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
        if validation::reference_shell(program, row.type_reference).is_some()
            && !row
                .address_projection
                .is_some_and(|projection| projection.forms_view())
        {
            return AddressReceiver::Reference {
                capture,
                projection,
            };
        }
        let Some(path) = row.address_projection else {
            // Remembering another value does not redirect this value's home.
            // The address owner separately authenticates its completed places.
            return AddressReceiver::InlineValue;
        };
        let Some(path) = facts.flow.control.operand_child_paths.span(path.path()) else {
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

/// Bind an observation to its immediate consumer's real evaluated operand.
/// Equal expressions elsewhere, later stored-output metadata and historical
/// supplier profiles cannot supply this receiving context.
pub(super) fn observed_reference_formation(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &checked_trees::FlowStateFact,
    statement: &checked_trees::FlowStatementFact,
    occurrence: &crate::checks::ownership_occurrences::StatementOccurrence,
    expression: typed_trees::expression::ExpressionHandle,
) -> Result<Option<ArgumentFormation>, &'static str> {
    use crate::checks::ownership_occurrences::{
        InvocationIdentity, InvocationOperandPosition, ObservationConsumer,
    };
    let control = &facts.flow.control;
    let root = if let Some(ObservationConsumer::Invocation(consumer)) = occurrence.consumer {
        let mut selected = None;
        for call in state.calls.handles() {
            let invocation = control.calls.get(call);
            if invocation.statement_index != statement.statement_index {
                continue;
            }
            let site = validation::find_call_site(
                program,
                state.machine_symbol,
                state.state_symbol,
                invocation.statement_index,
                invocation.call_ordinal,
            )
            .ok_or("the observation's invocation has no source owner")?;
            let target = validation::transition_call_target(
                program,
                program
                    .machine_by_symbol(state.machine_symbol)
                    .ok_or("the observation has no machine")?,
                program
                    .state_by_symbol(state.state_symbol)
                    .ok_or("the observation has no state")?,
                invocation.statement_index,
                invocation.call_ordinal,
            );
            if InvocationIdentity::from_call_site(site, target) != Some(consumer.invocation) {
                continue;
            }
            let borrowed = facts
                .borrow
                .call_at(
                    state.machine_symbol,
                    state.state_symbol,
                    invocation.statement_index,
                    invocation.call_ordinal,
                )
                .ok_or("the observation has no unique checked invocation")?;
            let borrowed = facts.borrow.calls.get(borrowed);
            if borrowed.target_symbol != invocation.target_symbol
                || borrowed.accesses != invocation.accesses
            {
                return Err(
                    "the observation's invocation has another selected target or access roster",
                );
            }
            let (_, offset) =
                crate::flow::captured_call_operands(program, &facts.flow, state, invocation)
                    .ok_or("the observation's invocation lost its original actual tuple")?;
            let actual = match consumer.position {
                InvocationOperandPosition::Argument(position) => {
                    invocation.argument_operands().handles().nth(position)
                }
                InvocationOperandPosition::Receiver if offset == 1 => {
                    Some(invocation.operands.start())
                }
                InvocationOperandPosition::Receiver => None,
            }
            .ok_or("the observation has no evaluated receiving operand")?;
            if selected.replace(actual).is_some() {
                return Err("the observation has multiple consuming invocations");
            }
        }
        selected.ok_or("the observation has no performed consuming invocation")?
    } else {
        match occurrence.consumer {
            Some(ObservationConsumer::StoredValue) => statement.stored_value,
            Some(ObservationConsumer::StoreTarget) => statement.store_target,
            Some(ObservationConsumer::EvaluatedValue) => statement.evaluated_value,
            Some(ObservationConsumer::OutgoingValue(target)) => {
                let mut outgoing = control
                    .outgoing
                    .span(state.outgoing)
                    .ok_or("the outgoing observation has no retained source roster")?
                    .iter()
                    .filter(|edge| {
                        edge.statement_index == statement.statement_index
                            && edge.transition_target == target
                    });
                let edge = outgoing
                    .next()
                    .ok_or("the outgoing observation has no evaluated value")?;
                if outgoing.next().is_some() {
                    return Err("the outgoing observation has multiple value consumers");
                }
                edge.returned_value
            }
            _ => return Err("the observation has no evaluated value consumer"),
        }
    };
    if !control.operands.is_valid(root) {
        return Err("the observation's value consumer has no completed capture");
    }
    let captures = evaluated_captures_on_route(program, facts, root, Some(&occurrence.route))?;
    let mut matching = captures
        .into_iter()
        .filter(|capture| control.operands.get(*capture).expression == expression);
    let capture = matching
        .next()
        .ok_or("the receiving operand has no matching evaluated observation")?;
    if matching.next().is_some() {
        return Err("the receiving operand has ambiguous evaluated observations");
    }
    reference_formation(program, facts, capture)
}

fn evaluated_captures(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    root: Handle<FlowOperandFact>,
) -> Result<Vec<Handle<FlowOperandFact>>, &'static str> {
    evaluated_captures_on_route(program, facts, root, None)
}

fn evaluated_captures_on_route(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    root: Handle<FlowOperandFact>,
    route: Option<&[crate::checks::ownership_occurrences::EvaluationBranch]>,
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
        let mut input_limit = row.evaluated_inputs.len();
        if let (Some(route), Some(conditional)) = (route, row.conditional_inputs_from) {
            let follows = match program.expression_table.expression(row.expression) {
                typed_trees::expression::ExpressionNode::Binary(binary) => {
                    let selected = binary.operator == typed_trees::expression::BinaryOperator::And;
                    route.iter().any(|branch| {
                        branch.expression() == row.expression && branch.selected() == selected
                    }) || crate::values::evaluate_closed_boolean_expression(
                        program,
                        &facts.operators,
                        binary.left,
                        &[],
                    ) == Some(selected)
                }
                _ => false,
            };
            if !follows {
                input_limit = conditional;
            }
        }
        pending.extend(
            row.evaluated_inputs
                .handles()
                .take(input_limit)
                .map(|value| (value, false)),
        );
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
            checked_trees::FlowOperandCompletion::Selected(alternatives) => {
                let alternatives = control
                    .operand_alternatives
                    .span(alternatives)
                    .ok_or("the actual argument has an unavailable alternative roster")?;
                if route.is_some()
                    && !matches!(
                        program.expression_table.expression(row.expression),
                        typed_trees::expression::ExpressionNode::Match(_)
                    )
                {
                    // Reusing a selected value does not execute its choice
                    // again. Its exact retained source owns the branch route;
                    // the copied completion still describes the yielded data.
                    if !control.operands.is_valid(row.backing) {
                        return Err("the transported selection lost its evaluated source");
                    }
                    let source = control.operands.get(row.backing);
                    if source.completion != row.completion
                        || source.state_symbol != owner.state_symbol
                        || source.statement_index != owner.statement_index
                    {
                        return Err("the transported selection has another completion owner");
                    }
                    continue;
                }
                for alternative in alternatives {
                    if let Some(route) = route {
                        let typed_trees::expression::ExpressionNode::Match(dispatch) =
                            program.expression_table.expression(row.expression)
                        else {
                            return Err("the selected operand has no matching source selection");
                        };
                        let arms =
                            crate::checks::multiplicity::reachable_arms(program, dispatch.arms);
                        let position = arms
                            .iter()
                            .position(|(handle, _)| *handle == alternative.source_arm)
                            .ok_or("the selected operand lost its original arm")?;
                        let branches = route
                            .iter()
                            .filter(|branch| branch.expression() == row.expression)
                            .collect::<Vec<_>>();
                        let pattern_position = branches
                            .iter()
                            .find(|branch| branch.selected())
                            .map(|branch| branch.decision())
                            .unwrap_or_else(|| {
                                branches
                                    .iter()
                                    .map(|branch| branch.decision() + 1)
                                    .max()
                                    .unwrap_or(0)
                            });
                        if position == pattern_position && alternative.pattern.is_valid() {
                            pending.push((alternative.pattern, false));
                        }
                        if !branches.iter().all(|branch| {
                            if branch.selected() {
                                position == branch.decision()
                            } else {
                                position > branch.decision()
                            }
                        }) {
                            continue;
                        }
                    }
                    pending.push((alternative.value, false));
                }
            }
            _ => {}
        }
    }
    Ok(seen)
}
