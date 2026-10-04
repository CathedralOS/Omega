//! Named transition arguments, read from the authored statement for the
//! nested call occurrence walk. Lowering's call lookups read stage 04's flow
//! call rows through `crate::call_sites` instead.

use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;
use checked_trees::CheckedTrees;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::statement::StatementNode;
use symbols::SymbolHandle;

pub(crate) mod nested;

/// Reconstruct a named terminal call from source, never from the operation
/// claiming it. No expression handle exists for this syntax: the statement
/// coordinate owns the invocation, and the explicit arguments keep theirs.
/// The invocation is an arm of the state's closing transition run: the
/// unconditional final arm a linear roster sequences, or a guarded arm whose
/// call runs only in that arm's completion block. Each plan owner rejoins
/// which placement it holds; this reconstruction checks only the source.
pub(super) fn named_terminal_arguments(
    checked: &CheckedTrees,
    caller_state: SymbolHandle,
    statement_index: u32,
) -> Result<(SymbolHandle, &[ExpressionHandle]), LoweringError> {
    use checked_trees::statement::{TransitionExit, TransitionGuardNode};
    let (machine, state) = crate::declarations::state_declaration(checked, caller_state)?;
    let statements = checked.statement_table.statements(state.statement_nodes);
    let Some(StatementNode::Transition(transition)) = statements.get(statement_index as usize)
    else {
        return unsupported("named call custody has no authored transition");
    };
    let unconditional_final = statement_index as usize == statements.len().saturating_sub(1)
        && transition.guard == TransitionGuardNode::Always;
    let closing_arm = statements[statement_index as usize..]
        .iter()
        .all(|statement| matches!(statement, StatementNode::Transition(_)));
    if !(unconditional_final || closing_arm)
        || transition.continuation.is_valid()
        || transition.exit != TransitionExit::Ordinary
        || !checked
            .statement_table
            .transition_target_is_valid(transition.target)
    {
        return unsupported("named call custody requires a terminal invocation arm");
    }
    let (source_target, explicit) =
        named_target_arguments(checked, machine.symbol, transition.target)?;
    // Naming the enclosing machine from a guarded arm re-enters its entry
    // state: a state-graph edge, never an arm's invocation.
    if !unconditional_final
        && crate::declarations::target_signature(checked, machine.symbol, source_target)?.machine
            == machine.symbol
    {
        return unsupported("machine re-entry in a guarded arm is a state edge");
    }
    Ok((source_target, explicit))
}

/// Read a selected named invocation at its exact source target. The caller's
/// occurrence walk owns guard/target/continuation ordering independently of
/// whether that invocation is terminal or conditional.
fn named_target_arguments(
    checked: &CheckedTrees,
    caller_machine: SymbolHandle,
    target: checked_trees::statement::TransitionTargetHandle,
) -> Result<(SymbolHandle, &[ExpressionHandle]), LoweringError> {
    if !checked.statement_table.transition_target_is_valid(target) {
        return unsupported("named call custody has a stale target");
    }
    let checked_trees::statement::TransitionTargetNode::Named {
        path,
        arguments,
        static_machine_parameter,
        ..
    } = checked.statement_table.transition_target(target)
    else {
        return unsupported("named call custody has no named target");
    };
    if static_machine_parameter.is_valid() {
        return unsupported("named call custody has an unselected static target");
    }
    let source_target = path.symbol;
    let target = crate::declarations::target_signature(checked, caller_machine, source_target)?;
    if target.machine == caller_machine && path.symbol != caller_machine {
        return unsupported("internal state transfer is not a terminal machine invocation");
    }
    let explicit = checked.statement_table.expression_handles(*arguments);
    if explicit.len() != arguments.count() as usize {
        return unsupported("named call custody has an invalid authored argument span");
    }
    Ok((source_target, explicit))
}
