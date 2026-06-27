use omega_core::symbols::SymbolHandle;
use omega_typed_trees::expression::ExpressionHandle;
use omega_typed_trees::machine::Machine;
use omega_typed_trees::state::State;
use omega_typed_trees::statement::{
    StatementNode, TransitionGuardNode, TransitionTargetHandle, TransitionTargetNode,
};

use super::facts::RangeFacts;
use super::guards::{seed_guard_facts, seed_negated_guard_facts};

/// A guard that provably holds at a state's entry: the state is reached by
/// exactly one incoming transition edge, and that edge is a guarded arm.
pub(super) struct IncomingGuard {
    state: SymbolHandle,
    guard: ExpressionHandle,
    /// True when the edge is the negated (continuation / `_`) arm.
    negated: bool,
}

struct Edge {
    target: SymbolHandle,
    /// `None` for an unguarded (`Always`) edge; `Some((guard, negated))` for a
    /// guarded arm.
    guard: Option<(ExpressionHandle, bool)>,
}

/// Collect, for every state reached by exactly ONE incoming edge that is a
/// guarded transition arm, the guard that therefore holds at its entry.
///
/// This lets a loop-head bound flow into the loop body across the state edge:
/// a body reached only from `transition self.i < self.text.len { true -> body }`
/// may then prove `self.text[self.i]`. Restricting to single-predecessor states
/// keeps the propagation sound WITHOUT a meet over predecessors -- if a state
/// has several incoming edges we cannot assume any single guard held on all of
/// them, so we seed nothing. Facts are keyed by symbol, so a guard naming a
/// source-state local (a different symbol than anything in the target) is inert
/// rather than unsound; only machine fields (`self.x`), shared across states,
/// actually carry.
pub(super) fn collect_incoming_guard_facts(
    program: &omega_typed_trees::TypedTrees,
    machine: &Machine,
) -> Vec<IncomingGuard> {
    let mut edges: Vec<Edge> = Vec::new();
    for state in program.machine_states(machine) {
        for statement in program.statement_table.statements(state.statement_nodes) {
            let StatementNode::Transition(transition) = statement else {
                continue;
            };
            match transition.guard {
                TransitionGuardNode::When(guard) if guard.is_valid() => {
                    push_edge(program, machine, &mut edges, transition.target, Some((guard, false)));
                    push_edge(
                        program,
                        machine,
                        &mut edges,
                        transition.continuation,
                        Some((guard, true)),
                    );
                }
                _ => {
                    push_edge(program, machine, &mut edges, transition.target, None);
                    push_edge(program, machine, &mut edges, transition.continuation, None);
                }
            }
        }
    }

    let mut result = Vec::new();
    for state in program.machine_states(machine) {
        let mut incoming = edges.iter().filter(|edge| edge.target == state.symbol);
        let (Some(edge), None) = (incoming.next(), incoming.next()) else {
            // Zero incoming edges (an entry state) or several (a join we cannot
            // soundly narrow without a meet): seed nothing.
            continue;
        };
        if let Some((guard, negated)) = edge.guard {
            result.push(IncomingGuard {
                state: state.symbol,
                guard,
                negated,
            });
        }
    }
    result
}

fn push_edge(
    program: &omega_typed_trees::TypedTrees,
    machine: &Machine,
    edges: &mut Vec<Edge>,
    target: TransitionTargetHandle,
    guard: Option<(ExpressionHandle, bool)>,
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
            target: path.symbol,
            guard,
        });
    }
}

/// Seed `facts` with the entry guard for `state`, when one was collected.
pub(super) fn seed_incoming_guard_facts(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    state: &State,
    incoming: &[IncomingGuard],
) {
    for entry in incoming.iter().filter(|entry| entry.state == state.symbol) {
        if entry.negated {
            seed_negated_guard_facts(program, facts, entry.guard);
        } else {
            seed_guard_facts(program, facts, entry.guard);
        }
    }
}
