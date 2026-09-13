//! Rejoin bound, temporary, and discarded result sources independently of operands.

use super::*;
use checked_trees::statement::StatementNode;

/// A discarded invocation produces a value but no source-local binding.
pub(crate) fn discards_result(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
) -> Result<bool, LoweringError> {
    let (_, source) = crate::scalar_source_custody::authored_state(checked, state)?;
    let statement = checked
        .statement_table
        .statements(source.statement_nodes)
        .get(coordinate.statement_index as usize)
        .ok_or(LoweringError::Unsupported(
            "call result has no authored statement",
        ))?;
    Ok(coordinate.call_ordinal == 0
        && matches!(statement, StatementNode::Call(call) if call.discards_result))
}

/// A discarded statement result has an operation-owned place, not a fabricated
/// local. Reconstruct its plain-owned custody from the exact authored call
/// signature; the continuation owner separately requires immediate disposal.
pub(crate) fn validate_discarded_structural(
    checked: &CheckedTrees,
    caller_machine: symbols::SymbolHandle,
    caller_state: symbols::SymbolHandle,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
    result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
) -> Result<(), LoweringError> {
    let (machine, state) = crate::scalar_source_custody::authored_state(checked, caller_state)?;
    let Some(StatementNode::Call(call)) = checked
        .statement_table
        .statements(state.statement_nodes)
        .get(coordinate.statement_index as usize)
    else {
        return unsupported("discarded structural result has no authored call statement");
    };
    let target = super::authored::target_signature(checked, caller_machine, call.target_symbol)?;
    if machine.symbol != caller_machine
        || coordinate.call_ordinal != 0
        || result.statement_index != coordinate.statement_index
        || !call.discards_result
        || checked
            .primitive_type_reference(target.return_type)
            .is_some()
        || checked
            .normalized_type_identity(target.return_type)
            .into_string()
            != result.type_identity
        || checked.type_multiplicity(target.return_type) != result.multiplicity
        || result.multiplicity == language_semantics::Multiplicity::Linear
        || !(validation::has_plain_owned_contents_with_numeric_constraints(
            &checked.typed,
            target.return_type,
        ) || validation::is_closed_primitive_array_type(&checked.typed, target.return_type))
    {
        return unsupported("discarded structural result disagrees with its authored signature");
    }
    Ok(())
}

pub(crate) fn validate_structural(
    checked: &CheckedTrees,
    caller_machine: symbols::SymbolHandle,
    caller_state: symbols::SymbolHandle,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
    result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
) -> Result<(), LoweringError> {
    let (machine, state) = crate::scalar_source_custody::authored_state(checked, caller_state)?;
    if coordinate.call_ordinal != 0 {
        let authored = super::authored::locate_source(checked, caller_state, coordinate)?;
        let Some(checked_trees::NominalMachineUseSite::Expression(expression)) =
            authored.source_site
        else {
            return unsupported("nested boundary structural result has no authored expression");
        };
        let target =
            super::authored::target_signature(checked, caller_machine, authored.source_target)?;
        if machine.symbol != caller_machine
            || result.statement_index != coordinate.statement_index
            || !((result.multiplicity == Multiplicity::Affine
                && validation::has_plain_owned_contents(&checked.typed, target.return_type))
                || (result.multiplicity == Multiplicity::Unrestricted
                    && !authored.boundary
                    && validation::is_closed_primitive_array_type(
                        &checked.typed,
                        target.return_type,
                    )))
            || checked
                .normalized_type_identity(target.return_type)
                .into_string()
                != result.type_identity
            || checked.type_multiplicity(target.return_type) != result.multiplicity
        {
            return unsupported("nested boundary result disagrees with its source custody");
        }
        return super::occurrences::validate(
            checked,
            caller_machine,
            caller_state,
            coordinate,
            expression,
        );
    }
    if matches!(
        checked
            .statement_table
            .statements(state.statement_nodes)
            .get(result.statement_index as usize),
        Some(StatementNode::Call(_))
    ) {
        return validate_discarded_structural(
            checked,
            caller_machine,
            caller_state,
            coordinate,
            result,
        );
    }
    let statements = checked.statement_table.statements(state.statement_nodes);
    if let Some(StatementNode::Expression(expression)) =
        statements.get(result.statement_index as usize)
    {
        if machine.symbol != caller_machine
            || result.statement_index != coordinate.statement_index
            || result.statement_index as usize + 1 != statements.len()
            || checked
                .normalized_type_identity(crate::attached_unit::structural_carrier_type(
                    checked,
                    state.return_type,
                )?)
                .as_str()
                != result.type_identity
            || checked.type_multiplicity(state.return_type) != result.multiplicity
        {
            return unsupported(
                "structural call result differs from its authored return destination",
            );
        }
        return super::occurrences::validate(
            checked,
            caller_machine,
            caller_state,
            coordinate,
            *expression,
        );
    }
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
        || !local.symbol.is_valid()
        || checked
            .normalized_type_identity(crate::attached_unit::structural_carrier_type(
                checked,
                local.type_reference,
            )?)
            .into_string()
            != result.type_identity
        || validation::reference_result_custody::result_multiplicity(
            &checked.typed,
            local.type_reference,
        ) != result.multiplicity
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
    if coordinate.call_ordinal == 0
        && matches!(checked.statement_table.statements(state.statement_nodes)
            .get(coordinate.statement_index as usize), Some(StatementNode::Call(call)) if call.discards_result)
    {
        if machine.symbol != caller_machine {
            return unsupported("discarded call belongs to another authored machine");
        }
        // Statement calls have no initializer expression. Rejoin the actual
        // statement; validate_operation still checks every computation root,
        // captured nested occurrence, target signature, and argument role.
        super::authored::locate_source(checked, caller_state, coordinate)?;
        return Ok(());
    }
    let statements = checked.statement_table.statements(state.statement_nodes);
    let expression = match statements.get(coordinate.statement_index as usize) {
        Some(StatementNode::LocalData(local)) => local.initial_value,
        // A final value-producing call has the same ordered operand custody as
        // an initializer. Its result destination is checked by scalar completion.
        Some(StatementNode::Expression(expression))
            if coordinate.call_ordinal == 0
                && coordinate.statement_index as usize + 1 == statements.len()
                && checked
                    .primitive_type_reference(state.return_type)
                    .is_some() =>
        {
            *expression
        }
        _ => return unsupported("computed result operands have no authored value-producing call"),
    };
    if machine.symbol != caller_machine
        || !validation::result_initializer_call_is_supported(&checked.typed, machine, expression)
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
        expression,
    )
}
