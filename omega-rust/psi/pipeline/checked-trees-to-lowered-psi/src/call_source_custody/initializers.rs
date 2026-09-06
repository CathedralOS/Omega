//! Rejoin the result-producing outer call separately from its operand graphs.

use super::*;
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::StatementNode;

pub(super) fn validate(
    checked: &CheckedTrees,
    caller_machine: symbols::SymbolHandle,
    caller_state: symbols::SymbolHandle,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
) -> Result<(), LoweringError> {
    let (machine, state) = crate::scalar_source_custody::authored_state(checked, caller_state)?;
    let Some(StatementNode::LocalData(local)) = checked
        .statement_table
        .statements(state.statement_nodes)
        .get(coordinate.statement_index as usize)
    else {
        return unsupported("computed result operands have no authored initializer");
    };
    if machine.symbol != caller_machine
        || !validation::unit_result_initializer_call_is_supported(
            &checked.typed,
            machine,
            local.initial_value,
        )
    {
        return unsupported(
            "computed result operands disagree with their authored initializer route",
        );
    }
    let ExpressionNode::Call(authored) = checked.expression_table.expression(local.initial_value)
    else {
        return unsupported("computed result initializer has no authored call");
    };
    let receiver = match checked.expression_table.expression(authored.receiver) {
        ExpressionNode::Name(path) if authored.receiver.is_valid() => path.symbol,
        _ => symbols::SymbolHandle::invalid(),
    };
    let control = &checked.facts.flow.control;
    let mut states = control
        .states
        .iter()
        .map(|(_, state)| state)
        .filter(|state| {
            state.machine_symbol == caller_machine && state.state_symbol == caller_state
        });
    let state = states.next().ok_or(LoweringError::Unsupported(
        "computed result initializer has no checked state",
    ))?;
    if states.next().is_some() {
        return unsupported("computed result initializer has ambiguous checked state");
    }
    let calls = control
        .calls
        .span(state.calls)
        .ok_or(LoweringError::Unsupported(
            "computed result initializer has an invalid call span",
        ))?;
    let mut calls = calls.iter().filter(|call| {
        call.statement_index == coordinate.statement_index as usize
            && call.call_ordinal == coordinate.call_ordinal as usize
    });
    let call = calls.next().ok_or(LoweringError::Unsupported(
        "computed result initializer has no checked outer call",
    ))?;
    if calls.next().is_some()
        || call.authored_expression != local.initial_value
        || call.has_receiver != authored.receiver.is_valid()
        || call.receiver_symbol != receiver
    {
        return unsupported(
            "computed result initializer disagrees with its captured outer occurrence",
        );
    }
    Ok(())
}
