//! Scalar completion rejoins the exact final source occurrence and its live
//! scalar binding. The statement schedule owns evaluation, effects, and custody.

use super::*;
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::StatementNode;

pub(super) fn validate(
    checked: &CheckedTrees,
    machine: &CheckedUnitEffectMachinePlan,
) -> Result<(), LoweringError> {
    let result = machine
        .scalar_result
        .as_ref()
        .ok_or(LoweringError::Unsupported(
            "scalar completion has no retained result",
        ))?;
    if machine.structural_result.is_some() {
        return unsupported("ordinary completion has both scalar and structural results");
    }
    let (source, state) = crate::scalar_source_custody::authored_state(checked, machine.state)?;
    let statements = checked
        .typed
        .statement_table
        .statements(state.statement_nodes);
    if source.symbol != machine.machine
        || checked.primitive_type_reference(state.return_type) != Some(result.primitive_type)
        || !matches!(machine.operations.last(), Some(CheckedUnitEffectOperationPlan::Complete {
            statement_index, ..
        }) if *statement_index as usize == statements.len())
    {
        return unsupported("scalar completion differs from its authored result signature");
    }
    let Some(StatementNode::Expression(expression)) = statements.last() else {
        return unsupported("scalar completion has no final authored expression");
    };
    if matches!(
        checked
            .type_reference_table
            .type_reference(state.return_type),
        checked_trees::types::TypeReferenceNode::Constrained { .. }
    ) {
        return unsupported("scalar operation completion requires result refinement evidence");
    }
    // Ordered scalar completion has no predicate-contract lowering yet. Keep
    // those bodies with their existing scalar-graph owner rather than dropping
    // requirements or guarantees when selecting an operation-body result.
    if !checked.machine_contracts(source).is_empty() || !checked.state_contracts(state).is_empty() {
        return unsupported("scalar operation completion cannot erase authored scalar contracts");
    }
    if result.statement_index as usize + 1 == statements.len()
        && !matches!(
            checked.expression_table.expression(*expression),
            ExpressionNode::Call(_)
        )
    {
        let mut producers = machine.operations.iter().filter(|operation| {
            matches!(operation, CheckedUnitEffectOperationPlan::EstablishScalarLocal {
                result: candidate, ..
            } if candidate == result)
        });
        if producers.next().is_none() || producers.next().is_some() {
            return unsupported("scalar completion has no unique final expression producer");
        }
        // The establishment consumer independently replays the Return-role
        // source binding, including its exact expression and scalar type.
        for statement_index in 0..statements.len() {
            if !machine.operations.iter().any(|operation| {
                super::scalar_arrays::source_statement(operation) == Some(statement_index as u32)
            }) {
                return unsupported("scalar result body omits an authored statement");
            }
        }
        return Ok(());
    }
    let (source_expression, named_return) = match checked.expression_table.expression(*expression) {
        ExpressionNode::Name(name) => {
            let Some(StatementNode::LocalData(local)) =
                statements.get(result.statement_index as usize)
            else {
                return unsupported("scalar completion name has no retained call declaration");
            };
            if local.is_mutable
                || !local.symbol.is_valid()
                || result.statement_index as usize + 1 >= statements.len()
                || name.symbol != local.symbol
                || name.head_symbol != local.symbol
                || checked
                    .expression_table
                    .name_path_members(name.members)
                    .len()
                    != 1
                || checked.primitive_type_reference(local.type_reference)
                    != Some(result.primitive_type)
            {
                return unsupported("scalar completion name differs from its exact call binding");
            }
            (local.initial_value, true)
        }
        ExpressionNode::Call(_) if result.statement_index as usize + 1 == statements.len() => {
            (*expression, false)
        }
        _ => return unsupported("scalar completion source is not its call or immutable binding"),
    };
    let mut producers = machine
        .operations
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::ScalarCall {
                coordinate,
                result: candidate,
                ..
            } if candidate == result => Some(*coordinate),
            _ => None,
        });
    let coordinate = producers.next().ok_or(LoweringError::Unsupported(
        "scalar completion has no exact ordinary call result",
    ))?;
    if producers.next().is_some()
        || coordinate.statement_index != result.statement_index
        || coordinate.call_ordinal != 0
    {
        return unsupported("scalar completion call producer is ambiguous or redirected");
    }
    let authored =
        crate::call_source_custody::authored::locate_source(checked, machine.state, coordinate)?;
    if authored.source_site
        != Some(checked_trees::NominalMachineUseSite::Expression(
            source_expression,
        ))
    {
        return unsupported("scalar completion call differs from its final source expression");
    }
    for statement_index in 0..statements.len() {
        if named_return && statement_index + 1 == statements.len() {
            continue;
        }
        if !machine.operations.iter().any(|operation| {
            super::scalar_arrays::source_statement(operation) == Some(statement_index as u32)
        }) {
            return unsupported("scalar result body omits an authored statement");
        }
    }
    Ok(())
}
