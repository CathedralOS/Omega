//! Exact normal completion coordinates retained by checked control flow.
//!
//! Machine-boundary completions and same-machine edges have distinct flow
//! channels. Cleanup and result custody must consume the same selected exit;
//! the end of the lexical body is not a terminal call's completion occurrence.
use checked_trees::{CheckFacts, FlowClaimJoinExit};
use symbols::SymbolHandle;

pub(in crate::execution::terminal_unit) fn state_exits(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
) -> Result<Vec<FlowClaimJoinExit>, &'static str> {
    let flow_state = facts
        .flow
        .control
        .state(machine, state.symbol)
        .ok_or("normal cleanup state has no retained control flow")?;
    let mut rows = Vec::new();
    for outgoing in facts
        .flow
        .control
        .outgoing
        .span_or_empty(flow_state.outgoing)
    {
        let kind = if !outgoing.transition_target.is_valid() {
            checked_trees::FlowClaimJoinExitKind::FinalExpression
        } else {
            let transition = program
                .statement_table
                .statement_at(state, outgoing.statement_index)
                .ok_or("normal cleanup edge has no source statement")?
                .as_transition()
                .ok_or("normal cleanup edge is not a direct transition")?;
            if transition.target == outgoing.transition_target {
                checked_trees::FlowClaimJoinExitKind::TransitionTarget
            } else if transition.continuation == outgoing.transition_target {
                checked_trees::FlowClaimJoinExitKind::TransitionContinuation
            } else {
                return Err("normal cleanup edge does not select the source transition");
            }
        };
        let exit = checked_trees::FlowClaimJoinExit {
            state_symbol: state.symbol,
            statement_index: outgoing.statement_index,
            kind,
        };
        if !is_statically_unreachable(program, state, exit)? && !rows.contains(&exit) {
            rows.push(exit);
        }
    }
    // Machine-boundary exits live in a second channel: a nested machine
    // transfer records its edge here rather than in the same-machine
    // outgoing set. It still names the same source transition arm and still
    // owes its authored cleanup coordinate.
    for outgoing in facts.flow.control.exits.span_or_empty(flow_state.exits) {
        if outgoing.state_symbol != state.symbol {
            return Err("normal cleanup exit belongs to another state");
        }
        let kind = if !outgoing.transition_target.is_valid() {
            checked_trees::FlowClaimJoinExitKind::FinalExpression
        } else {
            let transition = program
                .statement_table
                .statement_at(state, outgoing.statement_index)
                .ok_or("normal cleanup edge has no source statement")?
                .as_transition()
                .ok_or("normal cleanup edge is not a direct transition")?;
            let selected = if outgoing.is_continuation {
                transition.continuation
            } else {
                transition.target
            };
            if selected != outgoing.transition_target {
                return Err("normal cleanup edge does not select the source transition");
            }
            if outgoing.is_continuation {
                checked_trees::FlowClaimJoinExitKind::TransitionContinuation
            } else {
                checked_trees::FlowClaimJoinExitKind::TransitionTarget
            }
        };
        let exit = checked_trees::FlowClaimJoinExit {
            state_symbol: state.symbol,
            statement_index: outgoing.statement_index,
            kind,
        };
        if !is_statically_unreachable(program, state, exit)? && !rows.contains(&exit) {
            rows.push(exit);
        }
    }
    Ok(rows)
}

/// Flow retains every formed source arm. Executable cleanup uses only the
/// exact live type contexts, just as destination planning does. Missing context
/// is not evidence of unreachability; ordinary runtime alternatives stay live.
/// Resolve the exit's own source target before consulting that context so an
/// invalid coordinate cannot disappear as an apparently dead cleanup receipt.
pub(super) fn is_statically_unreachable(
    program: &dyn validation::CheckingInput,
    state: &typed_trees::state::State,
    exit: FlowClaimJoinExit,
) -> Result<bool, &'static str> {
    if exit.state_symbol != state.symbol {
        return Err("normal cleanup selection belongs to another state");
    }
    let continuation = match exit.kind {
        checked_trees::FlowClaimJoinExitKind::TransitionTarget => false,
        checked_trees::FlowClaimJoinExitKind::TransitionContinuation => true,
        _ => return Ok(false),
    };
    let transition = program
        .statement_table
        .statement_at(state, exit.statement_index)
        .ok_or("normal cleanup selection has no source statement")?
        .as_transition()
        .ok_or("normal cleanup selection is not a source transition")?;
    let target = if continuation {
        transition.continuation
    } else {
        transition.target
    };
    if !program.statement_table.transition_target_is_valid(target) {
        return Err("normal cleanup selection has no source target");
    }
    Ok(program.static_transition_type_context_is_unreachable(target))
}
