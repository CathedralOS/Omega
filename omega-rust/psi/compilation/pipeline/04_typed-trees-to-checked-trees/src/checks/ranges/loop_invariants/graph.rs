//! graph slice of loop_invariants.

use symbols::SymbolHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionTargetHandle, TransitionTargetNode};

#[derive(Clone)]
pub(super) struct Edge {
    pub(super) source: SymbolHandle,
    pub(super) target: SymbolHandle,
}

/// Build the directed state-transition edges, restricted to edges whose target
/// is a state of `machine`.
pub(super) fn build_edges(program: &typed_trees::TypedTrees, machine: &Machine) -> Vec<Edge> {
    let mut edges = Vec::new();
    for state in program.machine_states(machine) {
        for statement in program.statement_table.statements(state.statement_nodes) {
            let StatementNode::Transition(transition) = statement else {
                continue;
            };
            push_edge(
                program,
                machine,
                &mut edges,
                state.symbol,
                transition.target,
            );
            push_edge(
                program,
                machine,
                &mut edges,
                state.symbol,
                transition.continuation,
            );
        }
    }
    edges
}

pub(super) fn push_edge(
    program: &typed_trees::TypedTrees,
    machine: &Machine,
    edges: &mut Vec<Edge>,
    source: SymbolHandle,
    target: TransitionTargetHandle,
) {
    if !target.is_valid() {
        return;
    }
    let TransitionTargetNode::Named { path, .. } =
        program.statement_table.transition_target(target)
    else {
        return;
    };
    if program
        .machine_states(machine)
        .iter()
        .any(|state| state.symbol == path.symbol)
    {
        edges.push(Edge {
            source,
            target: path.symbol,
        });
    }
}

/// States reachable from `start` without entering `excluded` (includes `start`).
pub(super) fn reachable_from_excluding(
    edges: &[Edge],
    start: SymbolHandle,
    excluded: SymbolHandle,
) -> Vec<SymbolHandle> {
    if start == excluded {
        return Vec::new();
    }
    let mut reached: Vec<SymbolHandle> = vec![start];
    let mut frontier: Vec<SymbolHandle> = vec![start];

    while let Some(symbol) = frontier.pop() {
        for edge in edges.iter().filter(|edge| edge.source == symbol) {
            if edge.target != excluded && !reached.contains(&edge.target) {
                reached.push(edge.target);
                frontier.push(edge.target);
            }
        }
    }

    reached
}

/// The natural loop of the back edges into `head`: `head` plus every node from
/// which a `back_source` is reachable without passing through `head`.
pub(super) fn natural_loop(
    edges: &[Edge],
    head: SymbolHandle,
    back_sources: &[SymbolHandle],
) -> Vec<SymbolHandle> {
    let mut loop_nodes: Vec<SymbolHandle> = vec![head];
    let mut worklist: Vec<SymbolHandle> = back_sources.to_vec();

    while let Some(node) = worklist.pop() {
        if loop_nodes.contains(&node) {
            continue;
        }
        loop_nodes.push(node);
        for edge in edges.iter().filter(|edge| edge.target == node) {
            if edge.source != head && !loop_nodes.contains(&edge.source) {
                worklist.push(edge.source);
            }
        }
    }

    loop_nodes
}

pub(super) fn find_state(states: &[State], symbol: SymbolHandle) -> Option<&State> {
    states.iter().find(|state| state.symbol == symbol)
}
