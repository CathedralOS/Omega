//! Match local disposal to exact outgoing custody, not a lexical drop count.
//! Establishment and transfers determine whether each selected normal edge
//! still owns the root. Drop and disposition rows must cover that independent
//! remainder; deleting both rows cannot erase a live local's obligation.
use super::{CheckedTrees, LoweringError};
use checked_trees::statement::StatementNode;
use checked_trees::{FlowPermissionEvaluation, FlowStateOutgoingFact};
use language_semantics::{PermissionAccess, PermissionEventKind, PermissionEventSource};
use symbols::SymbolHandle;

pub(crate) fn selected_drop_evaluations(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: &checked_trees::state::State,
    local: symbols::SymbolHandle,
) -> Result<Vec<checked_trees::FlowPermissionEvaluation>, LoweringError> {
    use checked_trees::{FlowClaimJoinExitKind, FlowPermissionEvaluation};
    let invalid = || LoweringError::Unsupported("Unit graph local cleanup lost its selected exit");
    let mut flows = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .filter_map(|(_, flow)| {
            (flow.machine_symbol == machine && flow.state_symbol == state.symbol).then_some(flow)
        });
    let flow = flows.next().ok_or_else(invalid)?;
    if flows.next().is_some() {
        return Err(invalid());
    }
    let outgoing = checked
        .facts
        .flow
        .control
        .outgoing
        .span(flow.outgoing)
        .ok_or_else(invalid)?;
    let statements = checked.statement_table.statements(state.statement_nodes);
    let mut evaluations = Vec::new();
    let mut expected = Vec::new();
    for exit in outgoing {
        if exit.machine_symbol != machine || exit.state_symbol != state.symbol {
            return Err(invalid());
        }
        let evaluation = if exit.transition_target.is_valid() {
            FlowPermissionEvaluation::TransitionTarget(exit.transition_target)
        } else {
            FlowPermissionEvaluation::Statement
        };
        if remaining_on_exit(checked, machine, state.symbol, local, exit)? {
            if expected.contains(&evaluation) {
                return Err(invalid());
            }
            expected.push(evaluation);
        }
    }
    for (_, disposition) in
        checked
            .facts
            .contextual_cleanup
            .dispositions
            .iter()
            .filter(|(_, row)| {
                row.machine == machine
                    && row.state == state.symbol
                    && row.source == PermissionEventSource::StateExit
                    && row.root == facts::PlaceRoot::Symbol(local)
            })
    {
        if disposition.exit.state_symbol != state.symbol
            || !checked
                .facts
                .flow
                .ownership
                .segments
                .span(disposition.path)
                .ok_or_else(invalid)?
                .is_empty()
        {
            return Err(invalid());
        }
        let target = match disposition.exit.kind {
            FlowClaimJoinExitKind::FinalExpression => Default::default(),
            FlowClaimJoinExitKind::TransitionTarget
            | FlowClaimJoinExitKind::TransitionContinuation => {
                let Some(StatementNode::Transition(transition)) =
                    statements.get(disposition.exit.statement_index)
                else {
                    return Err(invalid());
                };
                if disposition.exit.kind == FlowClaimJoinExitKind::TransitionTarget {
                    transition.target
                } else {
                    transition.continuation
                }
            }
            FlowClaimJoinExitKind::CallResult { .. } => return Err(invalid()),
        };
        if outgoing
            .iter()
            .filter(|exit| {
                exit.machine_symbol == machine
                    && exit.state_symbol == state.symbol
                    && exit.statement_index == disposition.exit.statement_index
                    && exit.transition_target == target
            })
            .count()
            != 1
        {
            return Err(invalid());
        }
        let evaluation = if target.is_valid() {
            FlowPermissionEvaluation::TransitionTarget(target)
        } else {
            FlowPermissionEvaluation::Statement
        };
        if evaluations.contains(&evaluation) {
            return Err(invalid());
        }
        evaluations.push(evaluation);
    }
    if evaluations.len() != expected.len()
        || evaluations
            .iter()
            .any(|evaluation| !expected.contains(evaluation))
    {
        return Err(invalid());
    }
    Ok(evaluations)
}

fn remaining_on_exit(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    local: SymbolHandle,
    exit: &FlowStateOutgoingFact,
) -> Result<bool, LoweringError> {
    let invalid = || LoweringError::Unsupported("local outgoing custody has an invalid occurrence");
    let ownership = &checked.facts.flow.ownership;
    // Selected construction destinations establish through their exact receipt,
    // rather than a second unconditional owned Establish event. Its topology
    // and source transfers are validated by the selection's ordinary consumer.
    let mut establishments = ownership
        .owned_selections
        .iter()
        .filter(|(_, receipt)| {
            receipt.machine == machine
                && receipt.state == state
                && receipt.destination == local
                && receipt.statement_ordinal as usize <= exit.statement_index
        })
        .map(|(_, receipt)| receipt.statement_ordinal as usize);
    let mut established_at = establishments.next();
    if establishments.next().is_some() {
        return Err(invalid());
    }
    let mut transferred_at = None;
    for (_, event) in ownership.permissions.iter().filter(|(_, event)| {
        event.machine_symbol == machine
            && event.state_symbol == state
            && event.root == facts::PlaceRoot::Symbol(local)
            && event.access == PermissionAccess::Owned
            && event.kind != PermissionEventKind::AffineDrop
    }) {
        let statement = match event.source {
            PermissionEventSource::StateEntry => 0,
            PermissionEventSource::Statement { statement_index }
            | PermissionEventSource::Call {
                statement_index, ..
            } => statement_index,
            PermissionEventSource::StateExit => return Err(invalid()),
        };
        if statement > exit.statement_index {
            continue;
        }
        let selected = match event.evaluation {
            FlowPermissionEvaluation::Statement | FlowPermissionEvaluation::TransitionGuard => true,
            FlowPermissionEvaluation::TransitionTarget(target) => {
                statement == exit.statement_index && target == exit.transition_target
            }
            FlowPermissionEvaluation::Unknown => return Err(invalid()),
        };
        if !selected {
            continue;
        }
        let path = ownership
            .segments
            .span(event.segments)
            .ok_or_else(invalid)?;
        if !path.is_empty() {
            // A child transfer leaves the containing root's residual alive.
            continue;
        }
        match event.kind {
            PermissionEventKind::Establish => {
                if established_at.replace(statement).is_some() {
                    return Err(invalid());
                }
            }
            PermissionEventKind::Transfer | PermissionEventKind::Consume => {
                if transferred_at.replace(statement).is_some() {
                    return Err(invalid());
                }
            }
            PermissionEventKind::AffineDrop => unreachable!(),
        }
    }
    // This local's exact establishment is a declaration statement (or its
    // selected initializer receipt). Its own binding cannot be consumed by
    // that initializer. Compare occurrence coordinates, never arena order;
    // ordinary call/selection replay still authenticates the transfer operand.
    if let Some(transfer) = transferred_at {
        if established_at.is_none_or(|establishment| transfer <= establishment) {
            return Err(invalid());
        }
        return Ok(false);
    }
    Ok(established_at.is_some())
}
