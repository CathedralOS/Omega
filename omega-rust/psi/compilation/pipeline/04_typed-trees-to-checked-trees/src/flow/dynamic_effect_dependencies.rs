//! Narrow operational envelopes through completed selector values, never current storage.
//! Unknown predecessors retain the public envelope; every known alternative contributes.

use arena::Handle;
use checked_trees::{CheckFacts, FlowOperandFact};
use facts::PlaceSegment;
use flow_effects::{
    CapturedSelectorArgument, CapturedSelectorCall, CapturedSelectorCalls, CapturedSelectorOrigin,
};

pub(crate) fn derive(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
) -> Result<CapturedSelectorCalls, Vec<diagnostics::Diagnostic>> {
    let selections = validation::collect_dynamic_implementation_selections(program)?;
    let mut captured = CapturedSelectorCalls::default();
    for (_, state) in facts.flow.control.states.iter() {
        for call in facts.flow.control.calls.span_or_empty(state.calls) {
            let mut arguments = arena::HandleSpan::empty();
            for (source_position, operand) in call.argument_operands().handles().enumerate() {
                let value = facts.flow.control.operands.get(operand);
                if program.primitive_type_reference(value.type_reference)
                    != Some(typed_trees::types::PrimitiveType::Selector)
                {
                    continue;
                }
                let Some(origins) = selected_implementations(
                    program,
                    facts,
                    &selections,
                    operand,
                    &[],
                    &mut Vec::new(),
                ) else {
                    continue;
                };
                if origins.is_empty() {
                    continue;
                }
                let origins = captured.origins.insert_many(origins);
                arguments.push_contiguous(captured.arguments.insert(CapturedSelectorArgument {
                    source_position,
                    origins,
                }));
            }
            if !arguments.is_empty() {
                captured.calls.append(CapturedSelectorCall {
                    caller_state: state.state_symbol,
                    statement_index: call.statement_index,
                    call_ordinal: call.call_ordinal,
                    target: call.target_symbol,
                    arguments,
                });
            }
        }
    }
    Ok(captured)
}

fn selected_implementations(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    selections: &[validation::DynamicImplementationSelection],
    value: Handle<FlowOperandFact>,
    projection: &[PlaceSegment],
    active: &mut Vec<(Handle<FlowOperandFact>, Vec<PlaceSegment>)>,
) -> Option<Vec<CapturedSelectorOrigin>> {
    if !facts.flow.control.operands.is_valid(value) {
        return None;
    }
    if active
        .iter()
        .any(|(prior, path)| *prior == value && path == projection)
    {
        // A cycle contributes its ingress leaves; unknown leaves still refuse the union.
        return Some(Vec::new());
    }
    active.push((value, projection.to_vec()));
    let result = (|| {
        let capture = facts.flow.control.operands.get(value);
        if projection.is_empty() {
            if let Some(selection) = selections
                .iter()
                .find(|selection| selection.occurrence == capture.expression)
            {
                return Some(vec![CapturedSelectorOrigin::Implementation(
                    selection.implementation,
                )]);
            }
        }
        if projection.is_empty() {
            if let typed_trees::expression::ExpressionNode::Name(name) =
                program.expression_table.expression(capture.expression)
            {
                let state = program.state_by_symbol(capture.state_symbol)?;
                let source = validation::canonical_place_from_expression_in_state(
                    program,
                    capture.state_symbol,
                    capture.statement_index,
                    capture.expression,
                )?;
                let incoming = source.segments.is_empty()
                    && super::stored_values::at_snapshot(
                        program,
                        &facts.flow.control,
                        capture.storage_snapshot,
                        &source,
                        super::stored_values::StoredObservation::Contents,
                    )
                    .is_none();
                if incoming
                    && program
                        .state_parameters(state)
                        .iter()
                        .any(|parameter| parameter.symbol == name.symbol)
                    && program.primitive_type_reference(capture.type_reference)
                        == Some(typed_trees::types::PrimitiveType::Selector)
                {
                    let owner = program.machine_holding_state(state.symbol)?;
                    if program.machine_states(owner).first()?.symbol == state.symbol {
                        return Some(vec![CapturedSelectorOrigin::Parameter(name.symbol)]);
                    }
                    let position = program
                        .state_parameters(state)
                        .iter()
                        .position(|parameter| parameter.symbol == name.symbol)?;
                    let mut origins = Vec::new();
                    let mut found_arrival = false;
                    for (_, call) in facts
                        .flow
                        .control
                        .calls
                        .iter()
                        .filter(|(_, call)| call.target_symbol == state.symbol)
                    {
                        found_arrival = true;
                        let actual = call.argument_operands().handles().nth(position)?;
                        for origin in selected_implementations(
                            program,
                            facts,
                            selections,
                            actual,
                            &[],
                            active,
                        )? {
                            if !origins.contains(&origin) {
                                origins.push(origin);
                            }
                        }
                    }
                    return found_arrival.then_some(origins);
                }
            }
        }
        let Some(predecessors) =
            super::completed_values::predecessors(program, facts, value, projection)
        else {
            return returned_implementations(program, facts, selections, value, projection, active);
        };
        if predecessors.is_empty() {
            return None;
        }
        let mut implementations = Vec::new();
        for (predecessor, path) in predecessors {
            for implementation in
                selected_implementations(program, facts, selections, predecessor, &path, active)?
            {
                if !implementations.contains(&implementation) {
                    implementations.push(implementation);
                }
            }
        }
        Some(implementations)
    })();
    active.pop();
    result
}

fn returned_implementations(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    selections: &[validation::DynamicImplementationSelection],
    value: Handle<FlowOperandFact>,
    projection: &[PlaceSegment],
    active: &mut Vec<(Handle<FlowOperandFact>, Vec<PlaceSegment>)>,
) -> Option<Vec<CapturedSelectorOrigin>> {
    let capture = facts.flow.control.operands.get(value);
    let state = facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, state)| (state.state_symbol == capture.state_symbol).then_some(state))?;
    let mut calls = facts
        .flow
        .control
        .calls
        .span_or_empty(state.calls)
        .iter()
        .filter(|call| {
            capture.expression.is_valid()
                && call.authored_expression == capture.expression
                && call.statement_index == capture.statement_index
        });
    let call = calls.next()?;
    if calls.next().is_some() {
        return None;
    }
    let machine = program.machine_by_own_or_state_symbol(call.target_symbol)?;
    if machine.is_public
        || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
    {
        return None;
    }
    let entry = program.machine_states(machine).first()?;
    if call.target_symbol != entry.symbol && call.target_symbol != machine.symbol {
        return None;
    }
    let parameters = program.state_parameters(entry);
    let mut implementations = Vec::new();
    let mut found_exit = false;
    for (_, exit) in facts
        .flow
        .control
        .exits
        .iter()
        .filter(|(_, exit)| exit.machine_symbol == machine.symbol)
    {
        found_exit = true;
        for origin in selected_implementations(
            program,
            facts,
            selections,
            exit.returned_value,
            projection,
            active,
        )? {
            let origins = match origin {
                CapturedSelectorOrigin::Parameter(parameter) => {
                    let position = parameters
                        .iter()
                        .position(|entry| entry.symbol == parameter)?;
                    let actual = call.argument_operands().handles().nth(position)?;
                    selected_implementations(program, facts, selections, actual, &[], active)?
                }
                origin => vec![origin],
            };
            for origin in origins {
                if !implementations.contains(&origin) {
                    implementations.push(origin);
                }
            }
        }
    }
    (found_exit && !implementations.is_empty()).then_some(implementations)
}
