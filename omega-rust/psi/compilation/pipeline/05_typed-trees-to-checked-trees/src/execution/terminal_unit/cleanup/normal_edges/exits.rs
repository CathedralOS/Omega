//! Exact normal completion coordinates retained by checked control flow.
//!
//! Machine-boundary completions and same-machine edges have distinct flow
//! channels. Cleanup and result custody must consume the same selected exit;
//! the end of the lexical body is not a terminal call's completion occurrence.
use checked_trees::{CheckFacts, FlowClaimJoinExit};
use symbols::SymbolHandle;

pub(super) struct StateExit {
    pub(super) exit: FlowClaimJoinExit,
    completed_storage: arena::Handle<checked_trees::FlowStorageSnapshot>,
    pub(super) returned_value: arena::Handle<checked_trees::FlowOperandFact>,
}

pub(in crate::execution::terminal_unit) fn state_exits(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
) -> Result<Vec<FlowClaimJoinExit>, &'static str> {
    let mut exits = Vec::new();
    for row in state_exit_rows(program, facts, machine, state)? {
        if !exits.contains(&row.exit) {
            exits.push(row.exit);
        }
    }
    Ok(exits)
}

/// Named transitions return no value, but cleanup still needs the storage
/// left by their operands. Keep every storage snapshot for the exact selected edge; an
/// unavailable alternative cannot borrow another alternative's snapshot.
pub(in crate::execution::terminal_unit) fn state_exit_storage(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    exit: FlowClaimJoinExit,
) -> Result<Vec<arena::Handle<checked_trees::FlowStorageSnapshot>>, &'static str> {
    let mut snapshots = Vec::new();
    for row in state_exit_rows(program, facts, machine, state)? {
        if row.exit != exit {
            continue;
        }
        if !facts
            .flow
            .control
            .storage_snapshots
            .is_valid(row.completed_storage)
        {
            return Err("completion cleanup: the selected exit has no completed storage snapshot");
        }
        if !snapshots.contains(&row.completed_storage) {
            snapshots.push(row.completed_storage);
        }
    }
    if snapshots.is_empty() {
        return Err("completion cleanup: the selected exit has no retained flow occurrence");
    }
    Ok(snapshots)
}

pub(super) fn state_exit_rows(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
) -> Result<Vec<StateExit>, &'static str> {
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
        rows.push(StateExit {
            exit,
            completed_storage: outgoing.completed_storage,
            returned_value: outgoing.returned_value,
        });
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
        rows.push(StateExit {
            exit,
            completed_storage: outgoing.completed_storage,
            returned_value: outgoing.returned_value,
        });
    }
    Ok(rows)
}
