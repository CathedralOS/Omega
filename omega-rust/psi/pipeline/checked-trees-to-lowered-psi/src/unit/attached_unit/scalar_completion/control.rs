//! Reconstruct the authored tail independently of the producer's completion.
//! Prefix coverage and selected return roles prevent a plausible Boolean value
//! from authorizing swapped arms, omitted statements, or a different return.
use super::super::CheckedScalarExpressionRole;
use super::{
    CheckedTrees, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan, LoweringError,
    StatementNode, unsupported,
};
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
    let prefix = validate_tail(checked, machine.machine, machine.state, control)?;
    let (_, state) =
        crate::expression_preparation::source_custody::authored_state(checked, machine.state)?;
    let statements = checked.statement_table.statements(state.statement_nodes);
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

/// Completion is independent of how a body establishes its prefix values.
/// Descriptor calls and ordinary effects must replay the same authored exits.
pub(crate) fn validate_tail(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    control: &checked_trees::CheckedUnitScalarControlPlan,
) -> Result<usize, LoweringError> {
    let (source, state) =
        crate::expression_preparation::source_custody::authored_state(checked, state_symbol)?;
    if source.symbol != machine
        || checked.machine_states(source).len() != 1
        || checked.primitive_type_reference(state.return_type) != Some(control.primitive_type)
        || !checked.machine_contracts(source).is_empty()
        || !checked.state_contracts(state).is_empty()
    {
        return unsupported("ordered scalar control lost its exact source signature");
    }
    crate::scalar_graph::scalar_contracts::with_result_range(
        checked,
        state_symbol,
        0,
        &checked_trees::ClosedScalarValueContractPlan::default(),
    )?;
    let prefix = match &control.terminator {
        CheckedScalarStateTerminator::Return { statement_ordinal } => {
            unconditional(checked, state_symbol, control, *statement_ordinal)?
        }
        CheckedScalarStateTerminator::Guarded { arms, fallback } => {
            crate::expression_preparation::source_custody::guarded_exits::validate(
                checked,
                state_symbol,
                *arms,
                fallback.as_ref(),
            )?
        }
        CheckedScalarStateTerminator::Conditional { .. } => {
            conditional(checked, state_symbol, control)?
        }
        _ => return unsupported("ordered scalar completion requires a returning tail"),
    };
    Ok(prefix)
}

fn unconditional(
    checked: &CheckedTrees,
    state_symbol: symbols::SymbolHandle,
    control: &checked_trees::CheckedUnitScalarControlPlan,
    statement_ordinal: u32,
) -> Result<usize, LoweringError> {
    let (_, state) =
        crate::expression_preparation::source_custody::authored_state(checked, state_symbol)?;
    let statements = checked.statement_table.statements(state.statement_nodes);
    let prefix = statement_ordinal as usize;
    let tail = statements.get(prefix..).ok_or(LoweringError::Unsupported(
        "ordered scalar return has an absent source statement",
    ))?;
    // Locating a Return value alone does not establish unconditional control:
    // guarded transitions also carry Return-role expressions. Reconstruct the
    // complete final statement before permitting its value to execute directly.
    let complete_return = match tail {
        [StatementNode::Expression(_)] => true,
        [StatementNode::Transition(transition)] => {
            transition.exit == TransitionExit::Ordinary
                && transition.guard == TransitionGuardNode::Always
                && !transition.continuation.is_valid()
                && checked
                    .statement_table
                    .transition_target_is_valid(transition.target)
                && matches!(
                    checked.statement_table.transition_target(transition.target),
                    checked_trees::statement::TransitionTargetNode::Value(_)
                )
        }
        _ => false,
    };
    if !complete_return {
        return unsupported("ordered scalar return differs from its unconditional authored tail");
    }
    let located = crate::expression_preparation::source_custody::locate(
        checked,
        state_symbol,
        statement_ordinal,
        CheckedScalarExpressionRole::Return,
    )?;
    if located.primitive_type != control.primitive_type {
        return unsupported("ordered scalar return changed its source carrier");
    }
    Ok(prefix)
}

fn conditional(
    checked: &CheckedTrees,
    state_symbol: symbols::SymbolHandle,
    control: &checked_trees::CheckedUnitScalarControlPlan,
) -> Result<usize, LoweringError> {
    let (_, state) =
        crate::expression_preparation::source_custody::authored_state(checked, state_symbol)?;
    let statements = checked.statement_table.statements(state.statement_nodes);
    let CheckedScalarStateTerminator::Conditional {
        guard_statement_ordinal,
        when_true,
        when_false,
    } = &control.terminator
    else {
        return unsupported("ordered scalar completion requires a conditional return tail");
    };
    crate::scalar_graph::scalar_graph_lowering::branch_destinations::validate_coordinates(
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
                    || crate::expression_preparation::source_custody::guarded_exits::complementary(
                        checked,
                        state_symbol,
                        *guard_statement_ordinal,
                    )?)
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
        let located = crate::expression_preparation::source_custody::locate(
            checked,
            state_symbol,
            *statement_ordinal,
            role,
        )?;
        if located.primitive_type != control.primitive_type {
            return unsupported("ordered scalar return changed its source carrier");
        }
    }
    Ok(prefix)
}
