//! Completed operands are selected through their consumer, never a global expression search.
use arena::Handle;
use checked_trees::{FlowControlFacts, FlowOperandFact};

pub fn consumer_captures(
    control: &FlowControlFacts,
    state: symbols::SymbolHandle,
    statement: usize,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<Vec<Handle<FlowOperandFact>>> {
    let state = control
        .states
        .iter()
        .find_map(|(_, row)| (row.state_symbol == state).then_some(row))?;
    let mut pending = Vec::new();
    for row in control
        .statements
        .span_or_empty(state.statements)
        .iter()
        .filter(|row| row.statement_index == statement)
    {
        pending.extend([row.stored_value, row.store_target]);
        for invocation in control
            .operator_invocations
            .span(row.operator_invocations)?
        {
            pending.extend(invocation.operands.handles());
        }
        for observation in control
            .value_observations
            .span_or_empty(row.value_observations)
        {
            pending.extend([observation.result, observation.receiver]);
        }
    }
    for call in control
        .calls
        .span_or_empty(state.calls)
        .iter()
        .filter(|row| row.statement_index == statement)
    {
        pending.extend(call.operands.handles());
    }
    for outgoing in control
        .outgoing
        .span_or_empty(state.outgoing)
        .iter()
        .filter(|row| row.statement_index == statement)
    {
        pending.push(outgoing.returned_value);
    }
    for exit in control
        .exits
        .span_or_empty(state.exits)
        .iter()
        .filter(|row| row.statement_index == statement)
    {
        pending.push(exit.returned_value);
    }
    let mut visited = Vec::new();
    let mut selected = Vec::new();
    while let Some(handle) = pending.pop() {
        if !handle.is_valid() {
            continue;
        }
        if !control.operands.is_valid(handle) {
            return None;
        }
        if visited.contains(&handle) {
            continue;
        }
        visited.push(handle);
        let operand = control.operands.get(handle);
        if operand.state_symbol == state.state_symbol
            && operand.statement_index == statement
            && operand.expression == expression
        {
            selected.push(handle);
        }
        // The result's actual evaluation inputs are also completed consumers.
        // This finds a selected operand's own occurrence; it does not claim
        // that a conditional input executed on every completion route.
        pending.extend(operand.evaluated_inputs.handles());
        pending.extend(
            control
                .operand_children
                .span(operand.constructed_children())?
                .iter()
                .map(|child| child.value),
        );
        if let checked_trees::FlowOperandCompletion::Selected(alternatives) = operand.completion {
            let rows = control.operand_alternatives.span(alternatives)?;
            if rows.is_empty() || rows.iter().any(|row| !row.source_arm.is_valid()) {
                return None;
            }
            pending.extend(rows.iter().map(|row| row.value));
        }
    }
    (!selected.is_empty()).then_some(selected)
}

#[cfg(test)]
mod tests;
