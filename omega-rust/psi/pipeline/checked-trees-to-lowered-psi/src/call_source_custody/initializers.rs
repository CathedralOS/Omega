//! Rejoin the result-producing outer call separately from its operand graphs.

use super::*;
use checked_trees::statement::StatementNode;

pub(super) fn validate_structural(
    checked: &CheckedTrees,
    caller_machine: symbols::SymbolHandle,
    caller_state: symbols::SymbolHandle,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
    result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
) -> Result<(), LoweringError> {
    let (machine, state) = crate::scalar_source_custody::authored_state(checked, caller_state)?;
    let Some(StatementNode::LocalData(local)) = checked
        .statement_table
        .statements(state.statement_nodes)
        .get(result.statement_index as usize)
    else {
        return unsupported("boundary structural result has no authored local");
    };
    if machine.symbol != caller_machine
        || result.statement_index != coordinate.statement_index
        || coordinate.call_ordinal != 0
        || local.is_mutable
        || !local.symbol.is_valid()
        || checked
            .normalized_type_identity(local.type_reference)
            .into_string()
            != result.type_identity
        || checked.type_multiplicity(local.type_reference) != result.multiplicity
    {
        return unsupported("boundary structural result disagrees with its authored local");
    }
    super::occurrences::validate(
        checked,
        caller_machine,
        caller_state,
        coordinate,
        local.initial_value,
    )
}

pub(crate) fn validate(
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
        || !validation::result_initializer_call_is_supported(
            &checked.typed,
            machine,
            local.initial_value,
        )
    {
        return unsupported(
            "computed result operands disagree with their authored initializer route",
        );
    }
    super::occurrences::validate(
        checked,
        caller_machine,
        caller_state,
        coordinate,
        local.initial_value,
    )
}
