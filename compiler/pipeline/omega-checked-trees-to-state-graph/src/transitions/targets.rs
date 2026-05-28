use omega_checked_trees::CheckedTrees;
use omega_checked_trees::statement::{TableCall, TransitionTargetHandle, TransitionTargetNode};
use omega_core::diagnostics::Diagnostic;
use omega_state_graph::{PlannedTransitionTarget, StateKey};

use crate::segments::StateSegment;

pub(super) fn plan_transition_target(
    source_key: StateKey,
    segments: &[StateSegment],
    target: TransitionTargetHandle,
    program: &CheckedTrees,
) -> Result<PlannedTransitionTarget, Diagnostic> {
    if !target.is_valid() {
        return Ok(PlannedTransitionTarget::Terminal);
    }

    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named { path, arguments: _ }
            if is_local_transition_path(
                source_key,
                program.statement_table.name_path_members(path.members),
                path.head_symbol,
            ) =>
        {
            let members = program.statement_table.name_path_members(path.members);
            let name = members
                .last()
                .expect("named transition has a state")
                .clone();
            let symbol = path.symbol;
            let target = symbol
                .is_valid()
                .then(|| find_initial_segment_by_symbol(segments, symbol))
                .flatten()
                .or_else(|| find_initial_segment_by_name(segments, &name));
            let Some(target) = target else {
                if members.len() == 2 {
                    return Ok(PlannedTransitionTarget::Nested {
                        receiver_symbol: path.head_symbol,
                        state_symbol: path.symbol,
                        receiver: members[0].clone(),
                        state: members[1].clone(),
                    });
                }

                return Err(Diagnostic::error(format!(
                    "unknown state transition target `{name}`"
                )));
            };

            Ok(PlannedTransitionTarget::State {
                index: target.0,
                key: target.1.key,
                name,
            })
        }
        TransitionTargetNode::Named { path, arguments: _ } => {
            let members = program.statement_table.name_path_members(path.members);
            if members.len() == 2 {
                return Ok(PlannedTransitionTarget::Nested {
                    receiver_symbol: path.head_symbol,
                    state_symbol: path.symbol,
                    receiver: members[0].clone(),
                    state: members[1].clone(),
                });
            }

            Err(Diagnostic::error(format!(
                "unsupported transition target `{}`",
                display_transition_path(members)
            )))
        }
        TransitionTargetNode::SelfTarget => Ok(PlannedTransitionTarget::SelfTarget),
        TransitionTargetNode::Terminal | TransitionTargetNode::Value(_) => {
            Ok(PlannedTransitionTarget::Terminal)
        }
    }
}

pub(super) fn plan_call_target(
    source_key: StateKey,
    segments: &[StateSegment],
    call: &TableCall,
    program: &CheckedTrees,
) -> Result<PlannedTransitionTarget, Diagnostic> {
    let receiver = program.statement_table.name_path_members(call.receiver);

    if receiver.is_empty() || call.receiver_symbol == source_key.machine {
        let name = call.target.clone();
        let symbol = call.target_symbol;
        let target = symbol
            .is_valid()
            .then(|| find_initial_segment_by_symbol(segments, symbol))
            .flatten()
            .or_else(|| find_initial_segment_by_name(segments, &name));
        let Some(target) = target else {
            if receiver.len() == 1 && receiver[0].as_str() == "self" {
                return Ok(PlannedTransitionTarget::Nested {
                    receiver_symbol: call.receiver_symbol,
                    state_symbol: call.target_symbol,
                    receiver: receiver[0].clone(),
                    state: call.target.clone(),
                });
            }

            return Err(Diagnostic::error(format!(
                "unknown state call target `{name}`"
            )));
        };

        return Ok(PlannedTransitionTarget::State {
            index: target.0,
            key: target.1.key,
            name,
        });
    }

    let receiver = receiver.last().cloned().unwrap_or_default();
    Ok(PlannedTransitionTarget::Nested {
        receiver_symbol: call.receiver_symbol,
        state_symbol: call.target_symbol,
        receiver,
        state: call.target.clone(),
    })
}

pub(super) fn next_segment_target(
    source_key: StateKey,
    segments: &[StateSegment],
) -> Result<PlannedTransitionTarget, Diagnostic> {
    let next_key = StateKey {
        segment_index: source_key.segment_index + 1,
        ..source_key
    };
    let target = segments
        .iter()
        .enumerate()
        .find(|(_, segment)| segment.key == next_key)
        .ok_or_else(|| {
            Diagnostic::error("internal state-call continuation segment was not indexed")
        })?;

    Ok(PlannedTransitionTarget::State {
        index: target.0,
        key: target.1.key,
        name: target.1.name.clone(),
    })
}

fn is_local_transition_path(
    source_key: StateKey,
    path: &[omega_checked_trees::name::Identifier],
    head_symbol: omega_core::symbols::SymbolHandle,
) -> bool {
    path.len() == 1 || path.len() == 2 && head_symbol == source_key.machine
}

fn display_transition_path(path: &[omega_checked_trees::name::Identifier]) -> String {
    let mut display = String::new();

    for member in path {
        if !display.is_empty() {
            display.push('.');
        }
        display.push_str(member.as_str());
    }

    display
}

fn find_initial_segment_by_symbol(
    segments: &[StateSegment],
    symbol: omega_core::symbols::SymbolHandle,
) -> Option<(usize, &StateSegment)> {
    segments
        .iter()
        .enumerate()
        .find(|(_, segment)| segment.key.state == symbol && segment.key.segment_index == 0)
}

fn find_initial_segment_by_name<'segments>(
    segments: &'segments [StateSegment],
    name: &omega_checked_trees::name::Identifier,
) -> Option<(usize, &'segments StateSegment)> {
    segments
        .iter()
        .enumerate()
        .find(|(_, segment)| segment.key.segment_index == 0 && segment.name == *name)
}
