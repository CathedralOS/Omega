//! The returning tail of a scalar completion. Stage 04 plans the tail's
//! exits; this reads the statement prefix they begin after and refuses an
//! exit roster the completion has no route for, such as an ordered arm that
//! transfers or crashes instead of returning.
use super::{CheckedTrees, LoweringError, StatementNode, unsupported};
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

/// Read one state's value-only exit roster and return the prefix length the
/// roster begins after. Shared by the ordinary single-state completion above
/// and a state graph's `ReturnScalar` states, which own their own signature
/// checks.
pub(crate) fn validate_exits(
    checked: &CheckedTrees,
    state_symbol: symbols::SymbolHandle,
    control: &checked_trees::CheckedUnitScalarControlPlan,
) -> Result<usize, LoweringError> {
    let prefix = match &control.terminator {
        CheckedScalarStateTerminator::Return { statement_ordinal } => {
            unconditional(checked, state_symbol, *statement_ordinal)?
        }
        CheckedScalarStateTerminator::Guarded { arms, fallback } => {
            returning_exits(checked, *arms, fallback.as_ref())?[0].guard_statement_ordinal as usize
        }
        CheckedScalarStateTerminator::Conditional { .. } => {
            conditional(checked, state_symbol, control)?
        }
        _ => return unsupported("ordered scalar completion requires a returning tail"),
    };
    Ok(prefix)
}

/// The ordered guarded exits of a returning scalar tail, as stage 04 planned
/// them. Every arm and the fallback return a value; an ordered roster that
/// transfers or crashes has no route through the scalar completion.
pub(crate) fn returning_exits<'checked>(
    checked: &'checked CheckedTrees,
    arms: arena::HandleSpan<checked_trees::CheckedScalarGuardedExit>,
    fallback: Option<&CheckedScalarBranchDestination>,
) -> Result<&'checked [checked_trees::CheckedScalarGuardedExit], LoweringError> {
    let arms = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .guarded_exits
        .span(arms)
        .ok_or(LoweringError::Unsupported(
            "ordered scalar guard roster is stale",
        ))?;
    if arms.is_empty() {
        return unsupported("ordered scalar guard roster is empty");
    }
    if arms
        .iter()
        .map(|arm| &arm.destination)
        .chain(fallback)
        .any(|destination| {
            !matches!(
                destination,
                CheckedScalarBranchDestination::Return {
                    is_continuation: false,
                    ..
                }
            )
        })
    {
        return unsupported("ordered scalar exit transfers or crashes instead of returning");
    }
    Ok(arms)
}

fn unconditional(
    checked: &CheckedTrees,
    state_symbol: symbols::SymbolHandle,
    statement_ordinal: u32,
) -> Result<usize, LoweringError> {
    let (_, state) = crate::declarations::state_declaration(checked, state_symbol)?;
    let statements = checked.statement_table.statements(state.statement_nodes);
    let prefix = statement_ordinal as usize;
    if statements.get(prefix..).is_none() {
        return unsupported("ordered scalar return has an absent source statement");
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
    let Some(StatementNode::Transition(_)) = tail.first() else {
        return unsupported("ordered scalar completion has no authored guard");
    };
    let CheckedScalarBranchDestination::Return { .. } = when_false else {
        return unsupported("ordered scalar completion must retain a returning false arm");
    };
    for destination in [when_true, when_false] {
        let CheckedScalarBranchDestination::Return { .. } = destination else {
            return unsupported("ordered scalar completion cannot replace an authored transfer");
        };
    }
    Ok(prefix)
}
