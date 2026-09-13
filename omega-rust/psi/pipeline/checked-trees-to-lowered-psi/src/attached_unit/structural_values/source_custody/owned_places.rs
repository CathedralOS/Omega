//! Whole owned child places retain their selected source and existing storage.
use super::*;
use checked_trees::{CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan};

pub(crate) fn validate(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: u32,
    expression: ExpressionHandle,
    expected: checked_trees::types::TypeReferenceHandle,
    argument: &CheckedUnitStructuralArgumentPlan,
) -> Result<(), LoweringError> {
    let (owner, source_state) = crate::scalar_source_custody::authored_state(checked, state)?;
    let ExpressionNode::Name(name) = checked.expression_table.expression(expression) else {
        return unsupported("owned record operand lost its source name");
    };
    if owner.symbol != machine
        || !name.symbol.is_valid()
        || name.head_symbol != name.symbol
        || name.members.count() != 1
        || !argument.path.is_empty()
        || argument.access != checked_trees::CheckedStructuralAccess::Owned
    {
        return unsupported("owned record operand changed its whole source access");
    }
    let source = match argument.source {
        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
            let parameter = checked
                .state_parameters(source_state)
                .iter()
                .filter(|parameter| {
                    !parameter.is_const
                        && checked
                            .primitive_type_reference(parameter.type_reference)
                            .is_none()
                })
                .nth(parameter_index as usize)
                .ok_or(LoweringError::Unsupported(
                    "owned record operand parameter is missing",
                ))?;
            if parameter.symbol != name.symbol {
                return unsupported("owned record operand substituted its parameter");
            }
            parameter.type_reference
        }
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
            if symbol != name.symbol
                || checked
                    .state_parameters(source_state)
                    .iter()
                    .any(|parameter| parameter.symbol == symbol)
            {
                return unsupported("owned record operand substituted its local");
            }
            let mut locals = checked
                .statement_table
                .statements(source_state.statement_nodes)
                .iter()
                .enumerate()
                .filter_map(|(index, statement)| match statement {
                    StatementNode::LocalData(local) if local.symbol == symbol => {
                        Some((index, local))
                    }
                    _ => None,
                });
            let (index, local) = locals.next().ok_or(LoweringError::Unsupported(
                "owned record operand local is absent",
            ))?;
            if locals.next().is_some()
                || index >= statement as usize
                || !local.initial_value.is_valid()
            {
                return unsupported("owned record operand precedes its unique establishment");
            }
            local.type_reference
        }
        _ => return unsupported("owned record operand has no ordinary source place"),
    };
    if !matches!(
        checked.type_reference_table.type_reference(source),
        checked_trees::types::TypeReferenceNode::Named { .. }
    ) || !validation::has_plain_owned_contents_with_numeric_constraints(&checked.typed, source)
        || checked.normalized_type_identity(source) != checked.normalized_type_identity(expected)
        || checked.normalized_type_identity(source).as_str() != argument.type_identity
    {
        return unsupported("owned record operand substituted its declared carrier");
    }
    if checked.type_multiplicity(source) == language_semantics::Multiplicity::Affine {
        let ownership = &checked.facts.flow.ownership;
        let mut transfers = ownership
            .permissions
            .iter()
            .map(|(_, event)| event)
            .filter(|event| {
                event.machine_symbol == machine
                    && event.state_symbol == state
                    && event.root == facts::PlaceRoot::Symbol(name.symbol)
                    && event.kind == language_semantics::PermissionEventKind::Transfer
                    && event.source
                        == language_semantics::PermissionEventSource::Statement {
                            statement_index: statement as usize,
                        }
            });
        let transfer = transfers.next().ok_or(LoweringError::Unsupported(
            "owned record operand lost its statement transfer",
        ))?;
        if transfers.next().is_some()
            || transfer.access != language_semantics::PermissionAccess::Owned
            || transfer.multiplicity != language_semantics::Multiplicity::Affine
            || transfer.obligation_live
            || transfer.claim_identity != language_semantics::PermissionClaimIdentity::Unknown
            || ownership
                .segments
                .span(transfer.segments)
                .is_none_or(|path| !path.is_empty())
        {
            return unsupported("owned record operand changed its whole affine transfer");
        }
    }
    Ok(())
}
