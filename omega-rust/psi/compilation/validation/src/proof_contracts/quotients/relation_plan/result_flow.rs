//! Exact, non-authoritative quotient result-flow certificates.
//!
//! This module owns the bounded straight-line alias and finite forwarding-graph
//! judgments. It deliberately grants no effect, contract, `Respects`, custody,
//! checked-tree, or executable authority.

use crate::frozen_program::expression_table;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{
    StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::proof_contracts::quotients) struct ImmutableAliasFallthroughRoot {
    pub(in crate::proof_contracts::quotients) request_expression: ExpressionHandle,
    pub(in crate::proof_contracts::quotients) alias_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::proof_contracts::quotients) struct CompleteSingleStateResultFlow {
    pub(in crate::proof_contracts::quotients) machine_symbol: SymbolHandle,
    pub(in crate::proof_contracts::quotients) state_symbol: SymbolHandle,
    pub(in crate::proof_contracts::quotients) root: ImmutableAliasFallthroughRoot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::proof_contracts::quotients) struct CompleteStateForwardingResultFlow {
    pub(in crate::proof_contracts::quotients) machine_symbol: SymbolHandle,
    pub(in crate::proof_contracts::quotients) forwarding_edges: Vec<StateForwardingEdge>,
    pub(in crate::proof_contracts::quotients) result_state_symbol: SymbolHandle,
    pub(in crate::proof_contracts::quotients) root: ImmutableAliasFallthroughRoot,
}

/// The complete normal-result coverage judgment one request root earned: a
/// single transition-free state, or a finite convergent forwarding graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::proof_contracts::quotients) enum CompleteResultFlow {
    Single(CompleteSingleStateResultFlow),
    Forwarded(CompleteStateForwardingResultFlow),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::proof_contracts::quotients) struct StateForwardingEdge {
    pub(in crate::proof_contracts::quotients) source_state_symbol: SymbolHandle,
    pub(in crate::proof_contracts::quotients) target_state_symbol: SymbolHandle,
}

pub(in crate::proof_contracts::quotients) fn fallthrough_result_root(
    program: &dyn crate::CheckingInput,
    state: &State,
) -> Option<ImmutableAliasFallthroughRoot> {
    // A completing value is a transition whose target is that value. It is
    // the same result root as a trailing expression: the sealed request's
    // representative operation and theorem application stay on the call.
    let direct = program
        .statement_table
        .statements(state.statement_nodes)
        .last()
        .and_then(|statement| {
            let expression = result_expression(program, statement)?;
            is_quotient_request(program, expression).then_some(ImmutableAliasFallthroughRoot {
                request_expression: expression,
                alias_count: 0,
            })
        });
    direct.or_else(|| immutable_alias_fallthrough_root(program, state))
}

/// Recognize only the straight-line immutable alias form of one unchanged
/// state-fallthrough result. The final name may be a trailing expression or
/// the state's completing value. Named transfers, assignments, side
/// statements, mutable locals, and type drift stay excluded.
pub(in crate::proof_contracts::quotients) fn immutable_alias_fallthrough_root(
    program: &dyn crate::CheckingInput,
    state: &State,
) -> Option<ImmutableAliasFallthroughRoot> {
    if !state.return_type.is_valid() {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let (result, prefix) = statements.split_last()?;
    let result = result_expression(program, result)?;
    let mut expected_symbol = exact_local_name_symbol(program, result)?;
    let mut seen = Vec::new();
    for (position, statement) in prefix.iter().enumerate().rev() {
        let local = statement.as_local_data()?;
        if local.is_mutable
            || local.symbol != expected_symbol
            || seen.contains(&local.symbol)
            || !local.type_reference.is_valid()
            || program.normalized_type_identity(local.type_reference)
                != program.normalized_type_identity(state.return_type)
        {
            return None;
        }
        seen.push(local.symbol);
        match expression_table(program).expression(local.initial_value) {
            ExpressionNode::Call(call) if call.quotient_operation.is_some() && position == 0 => {
                return Some(ImmutableAliasFallthroughRoot {
                    request_expression: local.initial_value,
                    alias_count: seen.len(),
                });
            }
            ExpressionNode::Name(_) => {
                expected_symbol = exact_local_name_symbol(program, local.initial_value)?;
                if seen.contains(&expected_symbol) {
                    return None;
                }
            }
            _ => return None,
        }
    }
    None
}

/// Prove that the already-normalized fallthrough root exhausts normal result
/// exits for one exact state. A completing-value transition is that result;
/// every other transition leaves coverage incomplete. This says nothing about
/// effects, termination, contract correspondence, or executable admission.
pub(in crate::proof_contracts::quotients) fn complete_single_state_result_flow(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    root: ImmutableAliasFallthroughRoot,
) -> Option<CompleteSingleStateResultFlow> {
    if !machine.symbol.is_valid() || !state.symbol.is_valid() {
        return None;
    }
    let mut owner_matches = program
        .machines()
        .iter()
        .filter(|candidate| candidate.symbol == machine.symbol);
    let exact_owner = owner_matches.next()?;
    if exact_owner != machine || owner_matches.next().is_some() {
        return None;
    }
    let mut state_matches = program
        .machines()
        .iter()
        .flat_map(|candidate| program.machine_states(candidate))
        .filter(|candidate| candidate.symbol == state.symbol);
    let exact_state = state_matches.next()?;
    if exact_state != state || state_matches.next().is_some() {
        return None;
    }
    let states = program.machine_states(machine);
    if states.len() != 1
        || states[0].symbol != state.symbol
        || fallthrough_result_root(program, state) != Some(root)
        || has_disqualifying_transition(program, state)
    {
        return None;
    }
    Some(CompleteSingleStateResultFlow {
        machine_symbol: machine.symbol,
        state_symbol: state.symbol,
        root,
    })
}

/// Prove complete normal-result coverage for a finite forwarding graph. Every
/// non-result state must contain exactly one unconditional ordinary transition,
/// and every path must reach the state owning the unchanged quotient result
/// root. This grants no authority for conditional, cyclic, or mixed bodies.
pub(in crate::proof_contracts::quotients) fn complete_state_forwarding_result_flow(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    result_state: &State,
    root: ImmutableAliasFallthroughRoot,
) -> Option<CompleteStateForwardingResultFlow> {
    if !machine.symbol.is_valid() || !result_state.symbol.is_valid() {
        return None;
    }
    let exact_machine = program
        .machines()
        .iter()
        .filter(|candidate| candidate.symbol == machine.symbol)
        .collect::<Vec<_>>();
    if exact_machine.as_slice() != [machine] {
        return None;
    }
    let states = program.machine_states(machine);
    if states.len() < 2
        || typed_trees::state::state_by_symbol(states, result_state.symbol) != Some(result_state)
        || fallthrough_result_root(program, result_state) != Some(root)
        || has_disqualifying_transition(program, result_state)
    {
        return None;
    }
    for state in states {
        if !state.symbol.is_valid()
            || program
                .machines()
                .iter()
                .flat_map(|candidate| program.machine_states(candidate))
                .filter(|candidate| candidate.symbol == state.symbol)
                .count()
                != 1
        {
            return None;
        }
    }

    let mut edges = Vec::with_capacity(states.len() - 1);
    for forwarding_state in states
        .iter()
        .filter(|state| state.symbol != result_state.symbol)
    {
        let [StatementNode::Transition(transition)] = program
            .statement_table
            .statements(forwarding_state.statement_nodes)
        else {
            return None;
        };
        if transition.guard != typed_trees::statement::TransitionGuardNode::Always
            || transition.exit != typed_trees::statement::TransitionExit::Ordinary
            || transition.continuation.is_valid()
        {
            return None;
        }
        let typed_trees::statement::TransitionTargetNode::Named { path, .. } =
            program.statement_table.transition_target(transition.target)
        else {
            return None;
        };
        if !states.iter().any(|state| state.symbol == path.symbol) {
            return None;
        }
        edges.push((forwarding_state.symbol, path.symbol));
    }

    for state in states
        .iter()
        .filter(|state| state.symbol != result_state.symbol)
    {
        let mut current = state.symbol;
        let mut visited = Vec::new();
        while current != result_state.symbol {
            if visited.contains(&current) {
                return None;
            }
            visited.push(current);
            current = edges
                .iter()
                .find_map(|(source, target)| (*source == current).then_some(*target))?;
        }
    }

    Some(CompleteStateForwardingResultFlow {
        machine_symbol: machine.symbol,
        forwarding_edges: edges
            .iter()
            .map(|(source, target)| StateForwardingEdge {
                source_state_symbol: *source,
                target_state_symbol: *target,
            })
            .collect(),
        result_state_symbol: result_state.symbol,
        root,
    })
}

/// Dispatch the two complete normal-result coverage judgments: the two forms
/// are mutually exclusive because one requires exactly one state and the
/// other at least two.
pub(in crate::proof_contracts::quotients) fn complete_result_flow(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    result_state: &State,
    root: ImmutableAliasFallthroughRoot,
) -> Option<CompleteResultFlow> {
    complete_single_state_result_flow(program, machine, result_state, root)
        .map(CompleteResultFlow::Single)
        .or_else(|| {
            complete_state_forwarding_result_flow(program, machine, result_state, root)
                .map(CompleteResultFlow::Forwarded)
        })
}

/// The expression a statement contributes as the state's normal result.
///
/// A trailing expression and an unconditional completing-value transition
/// publish the same value. Named transfers, guards, continuations, and
/// patterned arms are not that result.
fn result_expression(
    program: &dyn crate::CheckingInput,
    statement: &StatementNode,
) -> Option<ExpressionHandle> {
    if let Some(expression) = statement.as_expression() {
        return Some(*expression);
    }
    let StatementNode::Transition(transition) = statement else {
        return None;
    };
    if transition.guard != TransitionGuardNode::Always
        || transition.exit != TransitionExit::Ordinary
        || transition.continuation.is_valid()
        || !transition.patterns.is_empty()
    {
        return None;
    }
    match program.statement_table.transition_target(transition.target) {
        TransitionTargetNode::Value(expression) if expression.is_valid() => Some(*expression),
        _ => None,
    }
}

fn is_quotient_request(program: &dyn crate::CheckingInput, expression: ExpressionHandle) -> bool {
    matches!(
        expression_table(program).expression(expression),
        ExpressionNode::Call(call) if call.quotient_operation.is_some()
    )
}

/// A completing-value transition is the result edge. Every other transition
/// — a guard, a named transfer, a continuation, or a patterned arm — leaves
/// normal-result coverage incomplete.
fn has_disqualifying_transition(program: &dyn crate::CheckingInput, state: &State) -> bool {
    let statements = program.statement_table.statements(state.statement_nodes);
    let last = statements.len().saturating_sub(1);
    statements.iter().enumerate().any(|(index, statement)| {
        matches!(statement, StatementNode::Transition(_))
            && (index != last || result_expression(program, statement).is_none())
    })
}

fn exact_local_name_symbol(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    let path = expression_table(program).name_path_node(expression)?;
    (path.symbol.is_valid()
        && expression_table(program)
            .name_path_members(path.members)
            .len()
            == 1)
        .then_some(path.symbol)
}
