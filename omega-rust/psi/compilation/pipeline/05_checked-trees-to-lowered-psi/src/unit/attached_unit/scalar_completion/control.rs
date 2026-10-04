//! Reconstruct the authored tail independently of the producer's completion.
//! Prefix coverage and selected return roles prevent a plausible Boolean value
//! from authorizing swapped arms, omitted statements, or a different return.
use super::super::CheckedScalarExpressionRole;
use super::{CheckedTrees, LoweringError, StatementNode, unsupported};
use checked_trees::statement::{TransitionExit, TransitionGuardNode};
use checked_trees::{CheckedScalarBranchDestination, CheckedScalarStateTerminator};

/// Completion is independent of how a body establishes its prefix values.
/// Descriptor calls and ordinary effects must replay the same authored exits.
/// This form admits no authored contract: its callers (forwarded helpers)
/// emit no guarantee obligations, so a contract here would be erased.
pub(crate) fn validate_tail(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    control: &checked_trees::CheckedUnitScalarControlPlan,
) -> Result<usize, LoweringError> {
    let (source, state) = crate::declarations::state_declaration(checked, state_symbol)?;
    if !checked.machine_contracts(source).is_empty() || !checked.state_contracts(state).is_empty() {
        return unsupported("ordered scalar control lost its exact source signature");
    }
    validate_signature_and_exits(checked, machine, state_symbol, control)
}

fn validate_signature_and_exits(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    control: &checked_trees::CheckedUnitScalarControlPlan,
) -> Result<usize, LoweringError> {
    let (source, state) = crate::declarations::state_declaration(checked, state_symbol)?;
    if source.symbol != machine
        || checked.machine_states(source).len() != 1
        || checked.primitive_type_reference(state.return_type) != Some(control.primitive_type)
    {
        return unsupported("ordered scalar control lost its exact source signature");
    }
    crate::scalar_graph::scalar_contracts::with_result_range(
        checked,
        state_symbol,
        0,
        &checked_trees::ClosedScalarValueContractPlan::default(),
    )?;
    validate_exits(checked, state_symbol, control)
}

/// Rejoin one state's value-only exit roster with its complete authored tail
/// and return the prefix length the roster begins after. Shared by the
/// ordinary single-state completion above and a state graph's `ReturnScalar`
/// states, which own their own signature checks.
pub(crate) fn validate_exits(
    checked: &CheckedTrees,
    state_symbol: symbols::SymbolHandle,
    control: &checked_trees::CheckedUnitScalarControlPlan,
) -> Result<usize, LoweringError> {
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
    let (_, state) = crate::declarations::state_declaration(checked, state_symbol)?;
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
    let (_, state) = crate::declarations::state_declaration(checked, state_symbol)?;
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
            rest @ ..,
        ] => !*is_continuation
            && !guard.continuation.is_valid()
            && !fallback.continuation.is_valid()
            && fallback.exit == TransitionExit::Ordinary
            && (fallback.guard == TransitionGuardNode::Always
                || crate::expression_preparation::source_custody::guard_complement::complementary(
                    checked,
                    state_symbol,
                    *guard_statement_ordinal,
                )?)
            // Reachable arms end at the second transition. Trailing arms are
            // the authored catch-alls an exhaustive pair leaves unreachable;
            // each must stay an ordinary, unconditioned, non-continuing
            // transfer exactly as the terminator's mint required.
            && rest.iter().all(|statement| {
                matches!(
                    statement,
                    StatementNode::Transition(dead)
                        if dead.guard == TransitionGuardNode::Always
                            && dead.exit == TransitionExit::Ordinary
                            && !dead.continuation.is_valid()
                )
            }),
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
