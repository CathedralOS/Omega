//! Exact, non-authoritative quotient result-flow certificates.
//!
//! This module owns the bounded straight-line alias and finite forwarding-graph
//! judgments. It deliberately grants no effect, contract, `Respects`, custody,
//! checked-tree, or executable authority.

use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;
use psi_typed_trees::statement::StatementNode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::quotients) struct ImmutableAliasFallthroughRoot {
    pub(in crate::quotients) request_expression: ExpressionHandle,
    pub(in crate::quotients) alias_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::quotients) struct CompleteSingleStateResultFlow {
    pub(in crate::quotients) machine_symbol: SymbolHandle,
    pub(in crate::quotients) state_symbol: SymbolHandle,
    pub(in crate::quotients) root: ImmutableAliasFallthroughRoot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::quotients) struct CompleteStateForwardingResultFlow {
    pub(in crate::quotients) machine_symbol: SymbolHandle,
    pub(in crate::quotients) forwarding_edges: Vec<StateForwardingEdge>,
    pub(in crate::quotients) result_state_symbol: SymbolHandle,
    pub(in crate::quotients) root: ImmutableAliasFallthroughRoot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::quotients) struct StateForwardingEdge {
    pub(in crate::quotients) source_state_symbol: SymbolHandle,
    pub(in crate::quotients) target_state_symbol: SymbolHandle,
}

pub(in crate::quotients) fn fallthrough_result_root(
    program: &TypedTrees,
    state: &State,
) -> Option<ImmutableAliasFallthroughRoot> {
    let direct = program
        .statement_table
        .statements(state.statement_nodes)
        .last()
        .and_then(|statement| {
            let StatementNode::Expression(expression) = statement else {
                return None;
            };
            matches!(
                program.expression_table.expression(*expression),
                ExpressionNode::Call(call) if call.quotient_operation.is_some()
            )
            .then_some(ImmutableAliasFallthroughRoot {
                request_expression: *expression,
                alias_count: 0,
            })
        });
    direct.or_else(|| immutable_alias_fallthrough_root(program, state))
}

/// Recognize only the straight-line immutable alias form of one unchanged
/// state-fallthrough result. This deliberately excludes transitions,
/// assignments, side statements, mutable locals, and type drift.
pub(in crate::quotients) fn immutable_alias_fallthrough_root(
    program: &TypedTrees,
    state: &State,
) -> Option<ImmutableAliasFallthroughRoot> {
    if !state.return_type.is_valid() {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let (StatementNode::Expression(result), prefix) = statements.split_last()? else {
        return None;
    };
    let mut expected_symbol = exact_local_name_symbol(program, *result)?;
    let mut seen = Vec::new();
    for (position, statement) in prefix.iter().enumerate().rev() {
        let StatementNode::LocalData(local) = statement else {
            return None;
        };
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
        match program.expression_table.expression(local.initial_value) {
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
/// exits only for one exact, transition-free state. This says nothing about
/// effects, termination, contract correspondence, or executable admission.
pub(in crate::quotients) fn complete_single_state_result_flow(
    program: &TypedTrees,
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
        || program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .any(|statement| matches!(statement, StatementNode::Transition(_)))
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
pub(in crate::quotients) fn complete_state_forwarding_result_flow(
    program: &TypedTrees,
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
        || states
            .iter()
            .find(|state| state.symbol == result_state.symbol)
            != Some(result_state)
        || fallthrough_result_root(program, result_state) != Some(root)
        || program
            .statement_table
            .statements(result_state.statement_nodes)
            .iter()
            .any(|statement| matches!(statement, StatementNode::Transition(_)))
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
        if transition.guard != psi_typed_trees::statement::TransitionGuardNode::Always
            || transition.exit != psi_typed_trees::statement::TransitionExit::Ordinary
            || transition.continuation.is_valid()
        {
            return None;
        }
        let psi_typed_trees::statement::TransitionTargetNode::Named { path, .. } =
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

fn exact_local_name_symbol(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    (path.symbol.is_valid()
        && program
            .expression_table
            .name_path_members(path.members)
            .len()
            == 1)
        .then_some(path.symbol)
}
