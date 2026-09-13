//! Reconstruct the authored tail independently of the producer's completion.
//! Prefix coverage and selected return roles prevent a plausible Boolean value
//! from authorizing swapped arms, omitted statements, or a different return.

use super::*;
use checked_trees::statement::{TransitionExit, TransitionGuardNode};
use checked_trees::{CheckedScalarBranchDestination, CheckedScalarStateTerminator};

pub(super) fn validate(
    checked: &CheckedTrees,
    machine: &CheckedUnitEffectMachinePlan,
) -> Result<(), LoweringError> {
    let control = machine
        .scalar_control
        .as_ref()
        .ok_or(LoweringError::Unsupported(
            "scalar control completion is absent",
        ))?;
    if machine.scalar_result.is_some() || machine.structural_result.is_some() {
        return unsupported("ordinary body has conflicting completion owners");
    }
    let (source, state) = crate::scalar_source_custody::authored_state(checked, machine.state)?;
    if source.symbol != machine.machine
        || checked.machine_states(source).len() != 1
        || checked.primitive_type_reference(state.return_type) != Some(control.primitive_type)
        || !checked.machine_contracts(source).is_empty()
        || !checked.state_contracts(state).is_empty()
    {
        return unsupported("ordered scalar control lost its exact source signature");
    }
    crate::scalar_contracts::with_result_range(
        checked,
        machine.state,
        0,
        &checked_trees::ClosedScalarValueContractPlan::default(),
    )?;
    let statements = checked.statement_table.statements(state.statement_nodes);
    let prefix = match &control.terminator {
        CheckedScalarStateTerminator::Guarded { arms, fallback } => {
            crate::scalar_source_custody::guarded_exits::validate(
                checked,
                machine.state,
                *arms,
                fallback.as_ref(),
            )?
        }
        CheckedScalarStateTerminator::Conditional { .. } => conditional(checked, machine, control)?,
        _ => return unsupported("ordered scalar completion requires a returning guard tail"),
    };
    if !matches!(machine.operations.last(), Some(CheckedUnitEffectOperationPlan::Complete {
        statement_index, ..
    }) if *statement_index as usize == statements.len())
    {
        return unsupported("ordered scalar completion omitted its final cleanup frontier");
    }
    for (statement, node) in statements.iter().enumerate().take(prefix) {
        if !matches!(
            node,
            StatementNode::LocalData(_) | StatementNode::Call(_) | StatementNode::Assignment(_)
        ) || !machine.operations.iter().any(|operation| {
            super::super::scalar_arrays::source_statement(operation) == Some(statement as u32)
        }) {
            return unsupported("ordered scalar control omitted an authored prefix statement");
        }
    }
    for operation in &machine.operations[..machine.operations.len() - 1] {
        if super::super::scalar_arrays::source_statement(operation)
            .is_none_or(|statement| statement as usize >= prefix)
        {
            return unsupported("ordered scalar control moved a tail effect into its prefix");
        }
    }
    Ok(())
}

fn conditional(
    checked: &CheckedTrees,
    machine: &CheckedUnitEffectMachinePlan,
    control: &checked_trees::CheckedUnitScalarControlPlan,
) -> Result<usize, LoweringError> {
    let (_, state) = crate::scalar_source_custody::authored_state(checked, machine.state)?;
    let statements = checked.statement_table.statements(state.statement_nodes);
    let CheckedScalarStateTerminator::Conditional {
        guard_statement_ordinal,
        when_true,
        when_false,
    } = &control.terminator
    else {
        return unsupported("ordered scalar completion requires a conditional return tail");
    };
    crate::scalar_graph_lowering::branch_destinations::validate_coordinates(
        *guard_statement_ordinal,
        when_true,
        when_false,
    )?;
    let prefix = *guard_statement_ordinal as usize;
    let tail = statements.get(prefix..).ok_or(LoweringError::Unsupported(
        "ordered scalar completion has an absent guard statement",
    ))?;
    let Some(StatementNode::Transition(guard)) = tail.first() else {
        return unsupported("ordered scalar completion has no authored guard");
    };
    if guard.exit != TransitionExit::Ordinary
        || !matches!(guard.guard, TransitionGuardNode::When(_))
    {
        return unsupported("ordered scalar completion substituted its guard exit");
    }
    let CheckedScalarBranchDestination::Return {
        is_continuation, ..
    } = when_false
    else {
        return unsupported("ordered scalar completion must retain a returning false arm");
    };
    let shape_matches = match tail {
        [StatementNode::Transition(_)] => *is_continuation && guard.continuation.is_valid(),
        [StatementNode::Transition(_), StatementNode::Expression(_)] => {
            !*is_continuation && !guard.continuation.is_valid()
        }
        [
            StatementNode::Transition(_),
            StatementNode::Transition(fallback),
        ] => {
            !*is_continuation
                && !guard.continuation.is_valid()
                && !fallback.continuation.is_valid()
                && fallback.exit == TransitionExit::Ordinary
                && (fallback.guard == TransitionGuardNode::Always
                    || complementary(checked, machine.state, *guard_statement_ordinal)?)
        }
        _ => false,
    };
    if !shape_matches {
        return unsupported("ordered scalar completion differs from its complete authored tail");
    }
    for destination in [when_true, when_false] {
        let CheckedScalarBranchDestination::Return {
            statement_ordinal,
            is_continuation,
        } = destination
        else {
            return unsupported("ordered scalar completion cannot replace an authored transfer");
        };
        let role = if *is_continuation {
            CheckedScalarExpressionRole::ContinuationReturn
        } else {
            CheckedScalarExpressionRole::Return
        };
        let located =
            crate::scalar_source_custody::locate(checked, machine.state, *statement_ordinal, role)?;
        if located.primitive_type != control.primitive_type {
            return unsupported("ordered scalar return changed its source carrier");
        }
    }
    Ok(prefix)
}

fn complementary(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    ordinal: u32,
) -> Result<bool, LoweringError> {
    use checked_trees::{CheckedBooleanExpression as Boolean, CheckedScalarExpression};
    let guard = |statement| {
        let (source, expression) = checked
            .facts
            .values
            .scalar_expressions
            .bound_expression_at(state, statement, CheckedScalarExpressionRole::Guard)
            .ok_or(LoweringError::Unsupported(
                "complementary fallback lost its pure source guard",
            ))?;
        crate::scalar_source_custody::validate_pure(checked, source, ScalarType::Boolean)?;
        let CheckedScalarExpression::Boolean(expression) = expression else {
            return unsupported("complementary fallback guard is not Boolean");
        };
        Ok(expression.as_ref())
    };
    fn base(mut expression: &Boolean) -> (&Boolean, bool) {
        let mut polarity = true;
        loop {
            match expression {
                Boolean::Not(operand) => {
                    expression = operand;
                    polarity = !polarity;
                }
                Boolean::Equal { left, right } => match (left.as_ref(), right.as_ref()) {
                    (Boolean::Constant(value), operand) | (operand, Boolean::Constant(value)) => {
                        expression = operand;
                        polarity = polarity == *value;
                    }
                    _ => return (expression, polarity),
                },
                _ => return (expression, polarity),
            }
        }
    }
    let next = ordinal
        .checked_add(1)
        .ok_or(LoweringError::Unsupported("fallback ordinal overflow"))?;
    let (left, left_polarity) = base(guard(ordinal)?);
    let (right, right_polarity) = base(guard(next)?);
    Ok(left == right && left_polarity != right_polarity)
}
